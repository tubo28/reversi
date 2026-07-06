use crate::reversi::bitboard::*;
use crate::reversi::hash::FxBuildHasher;
use crate::reversi::player::Player;
use crate::reversi::rand;
use std::cmp::{max, min};
use std::collections::HashMap;

/// Player by alpha-beta search, evolved from `AlphaBeta4Player` (whose tuned
/// `AlphaBeta4-2` weights it reuses verbatim). The *evaluation* is identical to
/// AB4-2; all of AB5's added strength is in the search, so it reads deeper for
/// the same wall-clock budget. Because the browser build (wasm32) has no usable
/// `std::time::Instant`, time is controlled by a deterministic **node budget**
/// rather than a clock: iterative deepening keeps going until the budget is
/// spent, then plays the best move from the deepest *completed* iteration.
///
/// Over AB4-2 it adds:
///  - node-budget iterative deepening to a much deeper nominal depth (AB4-2 was
///    a fixed depth 9); each odd depth is searched until the budget runs out,
///  - killer-move and history-heuristic move ordering on top of AB4's
///    corner/mobility ordering,
///  - bit-iteration move generation into a stack buffer (no per-node `Vec`),
///  - transposition tables carried across the whole game instead of being
///    rebuilt every move (the evaluation is a pure function of the board, so
///    entries stay valid), capped and cleared when they grow too large,
///  - a deeper exact endgame solver with fastest-first ordering, guarded by the
///    same budget with a fallback to the iterative-deepening search.
///
/// Kept as a separate `Player` so it can be measured head-to-head against
/// `AlphaBeta4-2` in `benches/duel5.rs`.
pub struct AlphaBeta5Player {
    rand: rand::Xor128,
    weights: PhaseWeights,
    /// Node budget for a single `next()` call. Deterministic (independent of the
    /// wall clock), so results are reproducible for a fixed seed and game line.
    budget: u64,
    search_tt: SearchTt,
    solve_tt: SolveTt,
    /// Two killer moves per ply (moves that caused a beta cutoff at that depth
    /// from the root); tried early before the general static ordering.
    killers: [[Mask; 2]; MAX_PLY],
    /// Per-square history score: how often a move from that square produced a
    /// cutoff. Halved every `next()` so it tracks the current phase.
    history: [i64; 64],
    nodes: u64,
    aborted: bool,
    /// Diagnostics for calibration (`benches/duel5.rs`): filled by `next()`.
    pub last_depth: usize,
    pub last_nodes: u64,
}

// Deepest nominal depth the iterative deepening will attempt. Like AB4 the depth
// must stay *odd* (this static evaluation has a strong even/odd tempo bias), so
// only odd depths 1, 3, ..., MAX_DEPTH are ever searched.
const MAX_DEPTH: usize = 13;

// Enough ply slots for killer indexing (search never recurses deeper than this).
const MAX_PLY: usize = 64;

// When this few empty cells remain, switch to the exact endgame solver. Kept low
// enough that the solve reliably finishes within the node budget (a deeper solve
// would just abort and waste the budget before falling back to the search). The
// iterative-deepening search already sees within a few cells of the end.
const ENDGAME_EMPTIES: u32 = 14;

// Aspiration half-width: after depth 5 the root searches a narrow window around
// the previous iteration's score, which lets the interior search prune far more
// aggressively. On a fail (score outside the window) we re-search full width.
const ASPIRATION_DELTA: i32 = 2500;

// Cap on transposition-table size (entries). Cleared past this to bound memory
// in a long game (~40 bytes/entry, so ~40 MB per table at the cap).
const TT_CAP: usize = 1_000_000;

// Default per-move node budget, sized so a browser (wasm) move takes about a
// second: native runs ~10-13M nodes/s and wasm is ~2-3x slower, so ~4M nodes is
// roughly a second in-browser. At this budget AB5 reaches ~depth 10 and beats
// the fixed-depth-9 `AlphaBeta4-2` (see `benches/duel5.rs`). Benches override it
// via `with_budget`.
const DEFAULT_NODE_BUDGET: u64 = 4_000_000;

// A enough large evaluate value.
const INF: i32 = 100_000_000;

// The four corner cells, tried first during move ordering.
const CORNERS: Mask = 0b_10000001_00000000_00000000_00000000_00000000_00000000_00000000_10000001;

// Files A (leftmost) and H (rightmost) columns. Used to stop horizontal bit
// shifts from wrapping around row boundaries when smearing into neighbours.
const NOT_FILE_A: Mask = 0xFEFEFEFEFEFEFEFE;
const NOT_FILE_H: Mask = 0x7F7F7F7F7F7F7F7F;

/// Kind of value stored in a transposition table entry, w.r.t. the search window.
#[derive(Clone, Copy)]
enum Bound {
    Exact,
    Lower,
    Upper,
}

#[derive(Clone, Copy)]
struct SearchEntry {
    depth: u8,
    value: i32,
    bound: Bound,
    best_move: Mask,
}

#[derive(Clone, Copy)]
struct SolveEntry {
    value: i32,
    bound: Bound,
    best_move: Mask,
}

// Transposition tables keyed by the raw (black, white) bitmasks, hashed with the
// fast FxHasher rather than the default SipHash.
type SearchTt = HashMap<(Mask, Mask), SearchEntry, FxBuildHasher>;
type SolveTt = HashMap<(Mask, Mask), SolveEntry, FxBuildHasher>;

/// Linear-combination weights for the evaluation terms (one set per game phase).
#[derive(Clone, Copy)]
pub struct Weights {
    pub pos: i32,
    pub mob: i32,
    pub pmob: i32,
    pub front: i32,
    pub stab: i32,
    pub disk: i32,
}

/// The three per-phase weight sets, selected by remaining empty cells.
#[derive(Clone, Copy)]
pub struct PhaseWeights {
    pub opening: Weights,
    pub midgame: Weights,
    pub endgame: Weights,
}

impl PhaseWeights {
    #[inline]
    fn select(&self, empties: u32) -> &Weights {
        if empties >= 40 {
            &self.opening
        } else if empties >= 20 {
            &self.midgame
        } else {
            &self.endgame
        }
    }
}

/// AB4-2's tuned weights (`pmob = 20/20/10`, `pos = 140/140/110` over AB4's
/// defaults). See `alphabeta42.rs` for the tuning history.
fn tuned_weights() -> PhaseWeights {
    PhaseWeights {
        opening: Weights { pos: 140, mob: 20, pmob: 20, front: 35, stab: 40, disk: 0 },
        midgame: Weights { pos: 140, mob: 15, pmob: 20, front: 25, stab: 70, disk: 0 },
        endgame: Weights { pos: 110, mob: 5, pmob: 10, front: 10, stab: 100, disk: 12 },
    }
}

impl AlphaBeta5Player {
    pub fn new(seed: u32) -> AlphaBeta5Player {
        Self::with_budget(seed, DEFAULT_NODE_BUDGET)
    }

    /// Same as `new` but with an explicit node budget, for calibration / duels.
    pub fn with_budget(seed: u32, budget: u64) -> AlphaBeta5Player {
        AlphaBeta5Player {
            rand: rand::Xor128::from_seed(seed),
            weights: tuned_weights(),
            budget,
            search_tt: SearchTt::default(),
            solve_tt: SolveTt::default(),
            killers: [[0; 2]; MAX_PLY],
            history: [0; 64],
            nodes: 0,
            aborted: false,
            last_depth: 0,
            last_nodes: 0,
        }
    }

    /// Records a beta cutoff for move ordering: promotes `mov` into the killer
    /// slots for `ply` and bumps its history score. Corner and TT moves are
    /// already ordered first, so they are excluded to keep the killer slots for
    /// the "quiet" moves that ordering would otherwise miss.
    #[inline]
    fn record_cutoff(&mut self, mov: Mask, ply: usize, depth: usize) {
        if mov & CORNERS == 0 && mov != self.killers[ply][0] {
            self.killers[ply][1] = self.killers[ply][0];
            self.killers[ply][0] = mov;
        }
        self.history[mov.trailing_zeros() as usize] += depth as i64;
    }

    /// One principal-variation search over the (already best-first ordered) root
    /// moves with window `[alpha0, beta]`. Returns `None` if the node budget ran
    /// out mid-iteration (discard the whole iteration); otherwise the best score,
    /// the best move, and every root move scored (for best-first re-ordering of
    /// the next, deeper iteration). The score may be a bound when it lands outside
    /// `[alpha0, beta]` (an aspiration fail), in which case the caller re-searches
    /// with a wider window and ignores the (incomplete) scored list.
    fn root_pvs(
        &mut self,
        board: &Board,
        moves: &[Mask],
        depth: usize,
        alpha0: i32,
        beta: i32,
    ) -> Option<(i32, Mask, Vec<(i32, Mask)>)> {
        let mut alpha = alpha0;
        let mut best = -INF;
        let mut best_move = moves[0];
        let mut scored: Vec<(i32, Mask)> = Vec::with_capacity(moves.len());
        for (i, &mov) in moves.iter().enumerate() {
            let child = board.flip(mov).switch();
            let score = if i == 0 {
                -self.search(&child, -beta, -alpha, depth, 0, false)
            } else {
                let s = -self.search(&child, -alpha - 1, -alpha, depth, 0, false);
                if s > alpha && s < beta {
                    -self.search(&child, -beta, -alpha, depth, 0, false)
                } else {
                    s
                }
            };
            if self.aborted {
                return None;
            }
            scored.push((score, mov));
            if score > best {
                best = score;
                best_move = mov;
            }
            if score > alpha {
                alpha = score;
            }
            if alpha >= beta {
                // Fail-high on the aspiration window: the scored list is now
                // incomplete, but the caller re-searches wider and discards it.
                break;
            }
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        Some((best, best_move, scored))
    }

    /// Negamax alpha-beta with principal-variation search. Returns 0 immediately
    /// once the node budget is exhausted (`self.aborted`); callers discard any
    /// result produced after that point.
    fn search(
        &mut self,
        board: &Board,
        alpha: i32,
        beta: i32,
        depth: usize,
        ply: usize,
        passed: bool,
    ) -> i32 {
        if self.aborted {
            return 0;
        }
        self.nodes += 1;
        if self.nodes >= self.budget {
            self.aborted = true;
            return 0;
        }
        debug_assert!(alpha <= beta);

        let black_moves = legal_moves(board.0, board.1);
        if depth == 0 || (black_moves == 0 && passed) {
            let white_moves = legal_moves(board.1, board.0);
            return Self::evaluate(board, &(black_moves, white_moves), &self.weights);
        }
        if black_moves == 0 {
            return -self.search(&board.switch(), -beta, -alpha, depth, ply, true);
        }

        let key = (board.0, board.1);
        let mut alpha = alpha;
        let mut beta = beta;
        let orig_alpha = alpha;

        let mut tt_move = 0;
        if let Some(&e) = self.search_tt.get(&key) {
            if e.depth as usize >= depth {
                match e.bound {
                    Bound::Exact => return e.value,
                    Bound::Lower => alpha = max(alpha, e.value),
                    Bound::Upper => beta = min(beta, e.value),
                }
                if alpha >= beta {
                    return e.value;
                }
            }
            tt_move = e.best_move;
        }

        let mut buf = [(0i32, 0u64); MAX_MOVES];
        let n = self.order_moves(board, black_moves, tt_move, ply, depth, &mut buf);

        let mut best = -INF;
        let mut best_move = buf[0].1;
        for i in 0..n {
            let mov = buf[i].1;
            let child = board.flip(mov).switch();
            let score = if i == 0 {
                -self.search(&child, -beta, -alpha, depth - 1, ply + 1, false)
            } else {
                // Null-window probe; re-search on a fail-high inside the window.
                let s = -self.search(&child, -alpha - 1, -alpha, depth - 1, ply + 1, false);
                if s > alpha && s < beta {
                    -self.search(&child, -beta, -alpha, depth - 1, ply + 1, false)
                } else {
                    s
                }
            };
            if self.aborted {
                return best;
            }
            if score > best {
                best = score;
                best_move = mov;
            }
            alpha = max(alpha, score);
            if alpha >= beta {
                self.record_cutoff(mov, ply, depth);
                break;
            }
        }

        // Never store a result computed under an abort (its value is garbage).
        if !self.aborted {
            let bound = if best <= orig_alpha {
                Bound::Upper
            } else if best >= beta {
                Bound::Lower
            } else {
                Bound::Exact
            };
            self.search_tt
                .insert(key, SearchEntry { depth: depth as u8, value: best, bound, best_move });
        }
        best
    }

    /// Fills `buf` with the legal moves in `moves_mask`, scored from most to
    /// least promising, and returns how many there are. Ordering tiers: the TT
    /// move, then corners, then killers, then fastest-first (fewest opponent
    /// replies) with a small history tiebreak. The opponent-mobility term is the
    /// costly part, so it is only computed at `depth >= 3` where it pays off.
    #[inline]
    fn order_moves(
        &self,
        board: &Board,
        moves_mask: Mask,
        tt_move: Mask,
        ply: usize,
        depth: usize,
        buf: &mut [(i32, Mask); MAX_MOVES],
    ) -> usize {
        let (k0, k1) = (self.killers[ply][0], self.killers[ply][1]);
        let use_mob = depth >= 3;
        let mut n = 0;
        let mut m = moves_mask;
        while m != 0 {
            let mov = m & m.wrapping_neg();
            m &= m - 1;
            let score = if mov == tt_move {
                i32::MAX
            } else {
                let mut s = 0;
                if mov & CORNERS != 0 {
                    s += 1_000_000;
                }
                if mov == k0 {
                    s += 500_000;
                } else if mov == k1 {
                    s += 400_000;
                }
                if use_mob {
                    let child = board.flip(mov).switch();
                    let opp = legal_moves(child.0, child.1).count_ones() as i32;
                    s -= opp * 1000;
                }
                // History as a sub-mobility-step tiebreak (mobility step is 1000).
                let h = self.history[mov.trailing_zeros() as usize];
                s += min(h, 900) as i32;
                s
            };
            buf[n] = (score, mov);
            n += 1;
        }
        buf[..n].sort_unstable_by(|a, b| b.0.cmp(&a.0));
        n
    }

    /// Exact endgame result for the side to move (`board.0`): the final disk
    /// difference (me − opp) under perfect play by both sides, or `None` if the
    /// node budget ran out before the exact tree was exhausted. A positive value
    /// is a *proven* forced win. Only affordable at low empty counts; used by the
    /// sprint generator to confirm a position is a guaranteed win. Never trust a
    /// `None` (aborted) result as a verdict.
    pub fn solve_exact(&mut self, board: &Board) -> Option<i32> {
        self.nodes = 0;
        self.aborted = false;
        let v = self.solve(board, -INF, INF, false);
        if self.aborted {
            None
        } else {
            Some(v)
        }
    }

    /// Exact endgame solver (PVS), returning the exact final *disk difference*
    /// (my disks − opp disks). Budget-guarded like `search`.
    fn solve(&mut self, board: &Board, alpha: i32, beta: i32, passed: bool) -> i32 {
        if self.aborted {
            return 0;
        }
        self.nodes += 1;
        if self.nodes >= self.budget {
            self.aborted = true;
            return 0;
        }
        debug_assert!(alpha <= beta);

        let my_moves = legal_moves(board.0, board.1);
        if my_moves == 0 {
            if passed {
                let (me, opp) = board.count();
                return me as i32 - opp as i32;
            }
            return -self.solve(&board.switch(), -beta, -alpha, true);
        }

        let key = (board.0, board.1);
        let mut alpha = alpha;
        let mut beta = beta;
        let orig_alpha = alpha;

        let mut tt_move = 0;
        if let Some(&e) = self.solve_tt.get(&key) {
            match e.bound {
                Bound::Exact => return e.value,
                Bound::Lower => alpha = max(alpha, e.value),
                Bound::Upper => beta = min(beta, e.value),
            }
            if alpha >= beta {
                return e.value;
            }
            tt_move = e.best_move;
        }

        // Fastest-first ordering (fewest opponent replies), skipping the mobility
        // computation in the last few plies where the subtree is tiny.
        let empties = 64 - (board.0 | board.1).count_ones();
        let use_mob = empties > 6;
        let mut buf = [(0i32, 0u64); MAX_MOVES];
        let mut n = 0;
        let mut m = my_moves;
        while m != 0 {
            let mov = m & m.wrapping_neg();
            m &= m - 1;
            let score = if mov == tt_move {
                i32::MAX
            } else {
                let mut s = 0;
                if mov & CORNERS != 0 {
                    s += 10_000;
                }
                if use_mob {
                    let child = board.flip(mov).switch();
                    s -= legal_moves(child.0, child.1).count_ones() as i32;
                }
                s
            };
            buf[n] = (score, mov);
            n += 1;
        }
        buf[..n].sort_unstable_by(|a, b| b.0.cmp(&a.0));

        let mut best = -INF;
        let mut best_move = buf[0].1;
        for i in 0..n {
            let mov = buf[i].1;
            let child = board.flip(mov).switch();
            let score = if i == 0 {
                -self.solve(&child, -beta, -alpha, false)
            } else {
                let s = -self.solve(&child, -alpha - 1, -alpha, false);
                if s > alpha && s < beta {
                    -self.solve(&child, -beta, -alpha, false)
                } else {
                    s
                }
            };
            if self.aborted {
                return best;
            }
            if score > best {
                best = score;
                best_move = mov;
            }
            alpha = max(alpha, score);
            if alpha >= beta {
                break;
            }
        }

        if !self.aborted {
            let bound = if best <= orig_alpha {
                Bound::Upper
            } else if best >= beta {
                Bound::Lower
            } else {
                Bound::Exact
            };
            self.solve_tt.insert(key, SolveEntry { value: best, bound, best_move });
        }
        best
    }

    /// Positional weight of `disks` using the same hand-tuned table as the
    /// baseline player (corners good, X/C squares bad).
    #[inline]
    fn positional(disks: Mask) -> i32 {
        // https://uguisu.skr.jp/othello/5-1.html
        const ADD30: Mask =
            0b_10000001_00000000_00000000_00000000_00000000_00000000_00000000_10000001;
        const SUB01: Mask =
            0b_00011000_00000000_00011000_10111101_10111101_00011000_00000000_00011000;
        const SUB03: Mask =
            0b_00000000_00111100_01000010_01000010_01000010_01000010_00111100_00000000;
        const SUB12: Mask =
            0b_01000010_10000001_00000000_00000000_00000000_00000000_10000001_01000010;
        const SUB16: Mask =
            0b_00000000_01000010_00000000_00000000_00000000_00000000_01000010_00000000;
        let mut w = 0;
        w += ((ADD30 & disks).count_ones() * 30) as i32;
        w -= (SUB01 & disks).count_ones() as i32;
        w -= ((SUB03 & disks).count_ones() * 3) as i32;
        w -= ((SUB12 & disks).count_ones() * 12) as i32;
        w -= ((SUB16 & disks).count_ones() * 16) as i32;
        w
    }

    /// Smears `disks` into all eight neighbouring directions, guarding the
    /// horizontal shifts against wrapping across row boundaries.
    #[inline]
    fn neighbours(disks: Mask) -> Mask {
        ((disks << 1) & NOT_FILE_A)
            | ((disks >> 1) & NOT_FILE_H)
            | (disks << 8)
            | (disks >> 8)
            | ((disks << 9) & NOT_FILE_A)
            | ((disks >> 9) & NOT_FILE_H)
            | ((disks << 7) & NOT_FILE_H)
            | ((disks >> 7) & NOT_FILE_A)
    }

    /// Counts frontier disks (disks adjacent to at least one empty cell) for
    /// black and white. Frontier disks are usually a liability.
    #[inline]
    fn frontier_counts(board: &Board) -> (u32, u32) {
        let Board(black, white) = *board;
        let empty = !(black | white);
        let neighbours = Self::neighbours(empty);
        ((black & neighbours).count_ones(), (white & neighbours).count_ones())
    }

    /// Potential mobility for black and white: the number of empty cells adjacent
    /// to the *opponent's* disks — squares where each side may gain future moves.
    #[inline]
    fn potential_mobility(board: &Board) -> (u32, u32) {
        let Board(black, white) = *board;
        let empty = !(black | white);
        let black_pmob = (Self::neighbours(white) & empty).count_ones();
        let white_pmob = (Self::neighbours(black) & empty).count_ones();
        (black_pmob, white_pmob)
    }

    /// Flood-fill stable-disk count for black and white. A disk is stable when it
    /// can never be flipped: along each of the four axes it is either on the
    /// board edge for that axis, part of a completely filled line, or flanked by
    /// an already-stable same-colored disk. Iterated to a fixpoint.
    #[inline]
    fn stable_full(board: &Board) -> (u32, u32) {
        let Board(black, white) = *board;
        let empty = !(black | white);
        const ALL: Mask = u64::MAX;

        #[inline]
        fn spread(bits: Mask, s: u32, gu: Mask, gd: Mask) -> Mask {
            let mut r = bits;
            let mut m = bits;
            for _ in 0..7 {
                m = (m << s) & gu;
                r |= m;
            }
            let mut m = bits;
            for _ in 0..7 {
                m = (m >> s) & gd;
                r |= m;
            }
            r
        }

        let full_h = !spread(empty, 1, NOT_FILE_A, NOT_FILE_H);
        let full_v = !spread(empty, 8, ALL, ALL);
        let full_d = !spread(empty, 9, NOT_FILE_A, NOT_FILE_H);
        let full_a = !spread(empty, 7, NOT_FILE_H, NOT_FILE_A);

        const FILE_A: Mask = 0x0101010101010101;
        const FILE_H: Mask = 0x8080808080808080;
        const RANK_1: Mask = 0x00000000000000FF;
        const RANK_8: Mask = 0xFF00000000000000;
        const BORDER: Mask = FILE_A | FILE_H | RANK_1 | RANK_8;

        let sh = full_h | FILE_A | FILE_H;
        let sv = full_v | RANK_1 | RANK_8;
        let sd = full_d | BORDER;
        let sa = full_a | BORDER;

        #[inline]
        fn stable_of(color: Mask, sh: Mask, sv: Mask, sd: Mask, sa: Mask) -> Mask {
            let mut stable = 0u64;
            loop {
                let safe_h = sh | ((stable << 1) & NOT_FILE_A) | ((stable >> 1) & NOT_FILE_H);
                let safe_v = sv | (stable << 8) | (stable >> 8);
                let safe_d = sd | ((stable << 9) & NOT_FILE_A) | ((stable >> 9) & NOT_FILE_H);
                let safe_a = sa | ((stable << 7) & NOT_FILE_H) | ((stable >> 7) & NOT_FILE_A);
                let next = color & safe_h & safe_v & safe_d & safe_a;
                if next == stable {
                    return stable;
                }
                stable = next;
            }
        }

        (
            stable_of(black, sh, sv, sd, sa).count_ones(),
            stable_of(white, sh, sv, sd, sa).count_ones(),
        )
    }

    /// Enhanced evaluation. Higher is better for black.
    #[inline]
    fn evaluate(board: &Board, moves: &(Mask, Mask), w: &PhaseWeights) -> i32 {
        let Board(black, white) = *board;
        let (black_moves, white_moves) = *moves;

        if black_moves == 0 && white_moves == 0 {
            let (b, w) = (black.count_ones(), white.count_ones());
            return match b.cmp(&w) {
                std::cmp::Ordering::Greater => INF,
                std::cmp::Ordering::Less => -INF,
                std::cmp::Ordering::Equal => 0,
            };
        }

        let empties = 64 - (black | white).count_ones();
        let wt = w.select(empties);

        let posdiff = Self::positional(black) - Self::positional(white);
        let mobdiff = black_moves.count_ones() as i32 - white_moves.count_ones() as i32;

        let (bpm, wpm) = Self::potential_mobility(board);
        let pmobdiff = bpm as i32 - wpm as i32;

        let (bf, wf) = Self::frontier_counts(board);
        let frontdiff = wf as i32 - bf as i32;

        let (bs, ws) = Self::stable_full(board);
        let stabdiff = bs as i32 - ws as i32;

        let diskdiff = black.count_ones() as i32 - white.count_ones() as i32;

        wt.pos * posdiff
            + wt.mob * mobdiff
            + wt.pmob * pmobdiff
            + wt.front * frontdiff
            + wt.stab * stabdiff
            + wt.disk * diskdiff
    }
}

// Upper bound on legal moves in any reachable position (the true max is ~33);
// sized generously so the ordering buffer never overflows.
const MAX_MOVES: usize = 48;

impl Player for AlphaBeta5Player {
    fn next(&mut self, board: &Board) -> Option<Mask> {
        let black_moves = legal_moves(board.0, board.1);
        if black_moves == 0 {
            return None;
        }

        // The search table must be rebuilt every move: its values come from the
        // tempo-biased static evaluation, so an entry stored at one move's search
        // parity is not a valid substitute at a later move's differing parity
        // (this is why AB4 also rebuilds it each move). The solve table is safe to
        // carry over — it stores the *exact* final disk difference, an intrinsic
        // property of the position with no depth/parity dependence — so it is only
        // cleared to bound memory in a long game.
        self.search_tt.clear();
        if self.solve_tt.len() > TT_CAP {
            self.solve_tt.clear();
        }
        for h in self.history.iter_mut() {
            *h /= 2;
        }
        self.killers = [[0; 2]; MAX_PLY];

        // Root moves, shuffled once so ties are broken uniformly at random.
        let mut moves: Vec<Mask> = Vec::new();
        let mut m = black_moves;
        while m != 0 {
            moves.push(m & m.wrapping_neg());
            m &= m - 1;
        }
        let n = moves.len();
        for i in 0..n - 1 {
            moves.swap(i, i + self.rand.next() as usize % (n - i));
        }

        let (black, white) = board.count();
        let empties = 64 - black - white;

        self.nodes = 0;
        self.aborted = false;

        // Endgame: one exact, disk-differential pass, budget-guarded. If it runs
        // out of budget we fall back to the iterative-deepening search below.
        if empties <= ENDGAME_EMPTIES {
            let mut alpha = -INF;
            let mut best_position = moves[0];
            for &mov in moves.iter() {
                let child = board.flip(mov).switch();
                let score = -self.solve(&child, -INF, -alpha, false);
                if self.aborted {
                    break;
                }
                if score > alpha {
                    alpha = score;
                    best_position = mov;
                }
            }
            if !self.aborted {
                self.last_depth = empties as usize;
                self.last_nodes = self.nodes;
                return Some(best_position);
            }
            // Solve timed out: reset for a fresh-budget iterative-deepening pass.
            self.aborted = false;
            self.nodes = 0;
        }

        // Iterative deepening over odd depths. Each iteration re-orders the root
        // moves best-first for the next, deeper pass, and (from depth 5 on)
        // searches a narrow aspiration window around the previous score so the
        // interior search prunes hard. The search stops when the node budget is
        // exhausted; we then play the best move from the deepest *completed*
        // iteration.
        let mut best_position = moves[0];
        let mut reached = 0;
        let mut prev_score = 0;
        let mut depth = 1;
        while depth <= MAX_DEPTH {
            // Aspirate around the previous score once it is stable; re-search with
            // a wider window on a fail-low/high until the score lands inside.
            let mut alpha = if depth >= 5 { prev_score - ASPIRATION_DELTA } else { -INF };
            let mut beta = if depth >= 5 { prev_score + ASPIRATION_DELTA } else { INF };
            let iteration = loop {
                match self.root_pvs(board, &moves, depth, alpha, beta) {
                    None => break None, // budget exhausted mid-iteration
                    Some((score, best_move, scored)) => {
                        if score <= alpha && alpha > -INF {
                            alpha = -INF; // fail-low: widen down and re-search
                            continue;
                        }
                        if score >= beta && beta < INF {
                            beta = INF; // fail-high: widen up and re-search
                            continue;
                        }
                        break Some((score, best_move, scored));
                    }
                }
            };
            let Some((score, best_move, scored)) = iteration else { break };
            prev_score = score;
            best_position = best_move;
            moves = scored.into_iter().map(|(_, mov)| mov).collect();
            reached = depth;
            depth += 2;
        }

        self.last_depth = reached;
        self.last_nodes = self.nodes;
        Some(best_position)
    }

    fn name(&self) -> &'static str {
        "Alpha-Beta5"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reversi::bitboard::{position_to_mask, Board};
    use crate::reversi::gm::{GameManager, Winner};
    use crate::reversi::player::random::RandomPlayer;

    #[test]
    fn frontier_of_initial_board() {
        let (b, w) = AlphaBeta5Player::frontier_counts(&Board::new());
        assert_eq!((b, w), (2, 2));
    }

    #[test]
    fn potential_mobility_initial_board_symmetric() {
        let (b, w) = AlphaBeta5Player::potential_mobility(&Board::new());
        assert_eq!(b, w);
        assert!(b > 0);
    }

    #[test]
    fn stable_full_detects_corners_only() {
        let corners = position_to_mask(0, 0)
            | position_to_mask(0, 7)
            | position_to_mask(7, 0)
            | position_to_mask(7, 7);
        let board = Board(corners, 0);
        assert_eq!(AlphaBeta5Player::stable_full(&board), (4, 0));
    }

    #[test]
    fn stable_full_full_board_all_stable() {
        let board = Board(u64::MAX, 0);
        assert_eq!(AlphaBeta5Player::stable_full(&board), (64, 0));
    }

    // --- Health gate: AlphaBeta5 must crush the random player. ---
    // Relative strength vs AlphaBeta4-2 is measured in `benches/duel5.rs`; this
    // is only a fast correctness gate, run with a small node budget for speed.

    const GAMES: u32 = 20;
    const MIN_WINS: u32 = 20;
    const TEST_BUDGET: u64 = 20_000;

    fn play_game(seed: u32, ab_is_black: bool) -> Result<(), (u32, Winner, (u32, u32))> {
        let ab = || Box::new(AlphaBeta5Player::with_budget(seed, TEST_BUDGET));
        let rand = || Box::new(RandomPlayer::new(seed.wrapping_add(1_000_000)));
        let (expected, mut gm) = if ab_is_black {
            (Winner::Black, GameManager::new(ab(), rand()))
        } else {
            (Winner::White, GameManager::new(rand(), ab()))
        };
        let result = gm.playout();
        if std::mem::discriminant(&result.winner) == std::mem::discriminant(&expected) {
            Ok(())
        } else {
            Err((seed, result.winner.clone(), result.disks))
        }
    }

    fn assert_dominates(ab_is_black: bool) {
        let threads =
            std::thread::available_parallelism().map(|n| (n.get() - 1).max(1)).unwrap_or(1);
        let per_thread: Vec<Vec<(u32, Winner, (u32, u32))>> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..threads)
                .map(|t| {
                    s.spawn(move || {
                        let mut losses = Vec::new();
                        let mut seed = t as u32;
                        while seed < GAMES {
                            if let Err(loss) = play_game(seed, ab_is_black) {
                                losses.push(loss);
                            }
                            seed += threads as u32;
                        }
                        losses
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });

        let mut losses: Vec<_> = per_thread.into_iter().flatten().collect();
        losses.sort_by_key(|&(seed, _, _)| seed);
        let wins = GAMES - losses.len() as u32;
        let side = if ab_is_black { "black" } else { "white" };
        assert!(
            wins >= MIN_WINS,
            "Alpha-Beta5 ({side}) won only {wins}/{GAMES} (need >= {MIN_WINS}); lost: {losses:?}"
        );
    }

    #[test]
    fn beats_random_as_black_almost_always() {
        assert_dominates(true);
    }

    #[test]
    fn beats_random_as_white_almost_always() {
        assert_dominates(false);
    }
}
