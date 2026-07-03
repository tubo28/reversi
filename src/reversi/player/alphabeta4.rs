use crate::reversi::bitboard::*;
use crate::reversi::hash::FxBuildHasher;
use crate::reversi::player::*;
use crate::reversi::{H, W};
use std::cmp::min;
use std::collections::HashMap;

/// Player by alpha-beta search, evolved from `AlphaBeta3Player`. It keeps AB3's
/// search core (FxHash transposition table, PVS/NegaScout, flood-fill stability,
/// phase-dependent weights) and adds three well-known engine techniques:
///  - iterative deepening with best-first root re-ordering and a transposition
///    table shared across the deepening iterations, so alpha-beta actually prunes
///    at the root (AB3 searched root moves in shuffled order from -INF),
///  - a potential-mobility evaluation term (empty cells adjacent to the
///    opponent's discs — squares where future moves may appear),
///  - an exact endgame solver that maximises the final *disc difference* instead
///    of only the win/loss/draw sign, entered a few plies earlier.
///
/// It is intentionally a separate `Player` so it can be measured head-to-head
/// against `AlphaBeta3` in `benches/duel.rs`.
pub struct AlphaBeta4Player {
    rand: rand::Xor128,
    weights: PhaseWeights,
}

// Same odd nominal depth as AB3. Depth must stay *odd*: this static evaluation
// has a strong even/odd (tempo) bias, and an even search depth plays worse. The
// iterative deepening below only ever visits odd depths for the same reason.
const SEARCH_DEPTH: usize = 9;

// When this few empty cells remain, switch to the exact endgame solver. Deeper
// than AB3's 12 — the disc-differential solver below still prunes hard enough
// (and the faster root search leaves budget for it).
const ENDGAME_EMPTIES: u32 = 16;

// A enough large evaluate value.
const INF: i32 = 100_000_000;

// The four corner cells, tried first during move ordering.
const CORNERS: Mask =
    0b_10000001_00000000_00000000_00000000_00000000_00000000_00000000_10000001;

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
    /// Positional weight: the hand-tuned square table (corners good, X/C squares
    /// bad). See `positional`.
    pub pos: i32,
    /// Current mobility: difference in the number of legal moves available now.
    pub mob: i32,
    /// Potential mobility: difference in empty cells adjacent to the opponent's
    /// discs — squares where future moves may appear. See `potential_mobility`.
    pub pmob: i32,
    /// Frontier: difference in discs adjacent to an empty cell (fewer is better,
    /// so the term is white − black). See `frontier_counts`.
    pub front: i32,
    /// Stability: difference in discs that can never be flipped. See `stable_full`.
    pub stab: i32,
    /// Raw disc count difference. Only matters near the end (kept at 0 until the
    /// endgame phase).
    pub disc: i32,
}

/// The three per-phase weight sets, selected by remaining empty cells. Injectable
/// so `benches/tune.rs` can sweep candidate weightings against a fixed opponent.
#[derive(Clone, Copy)]
pub struct PhaseWeights {
    pub opening: Weights,
    pub midgame: Weights,
    pub endgame: Weights,
}

impl Default for PhaseWeights {
    fn default() -> Self {
        // Carried over from AB3's tuned "more-stability" weights, plus a new
        // potential-mobility term (`pmob`) seeded as a weaker mirror of current
        // mobility. `pmob` still wants confirming/tuning against AB3 in the duel.
        PhaseWeights {
            // Opening: mobility and frontier dominate; raw disc count is irrelevant.
            opening: Weights { pos: 100, mob: 20, pmob: 15, front: 35, stab: 40, disc: 0 },
            // Midgame: positional table and stability lead, mobility still matters.
            midgame: Weights { pos: 100, mob: 15, pmob: 10, front: 25, stab: 70, disc: 0 },
            // Late: stability and disc count dominate, mobility fades.
            endgame: Weights { pos: 80, mob: 5, pmob: 3, front: 10, stab: 100, disc: 12 },
        }
    }
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

impl AlphaBeta4Player {
    pub fn new(seed: u32) -> AlphaBeta4Player {
        AlphaBeta4Player { rand: rand::Xor128::from_seed(seed), weights: PhaseWeights::default() }
    }

    /// Same as `new` but with an explicit weighting, for tuning experiments.
    pub fn with_weights(seed: u32, weights: PhaseWeights) -> AlphaBeta4Player {
        AlphaBeta4Player { rand: rand::Xor128::from_seed(seed), weights }
    }

    /// Orders the moves in `moves_mask` from most to least promising so that
    /// alpha-beta cuts off as early as possible.
    fn ordered_moves(
        board: &Board,
        moves_mask: Mask,
        parts: &[(Mask, Mask); 4],
        tt_move: Mask,
    ) -> Vec<Mask> {
        let mut scored: Vec<(i32, Mask)> = (0..H * W)
            .map(|i| 1u64 << i)
            .filter(|&mov| mov & moves_mask == mov)
            .map(|mov| {
                let score = if mov == tt_move {
                    i32::MAX
                } else {
                    let child = board.flip_with_hints(mov, parts).switch();
                    let (opp_moves, _) = child.get_valid_mask();
                    let mut s = -(opp_moves.count_ones() as i32);
                    if mov & CORNERS != 0 {
                        s += 1000;
                    }
                    s
                };
                (score, mov)
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.into_iter().map(|(_, mov)| mov).collect()
    }

    /// Negamax alpha-beta with principal-variation search: the first (best-ordered)
    /// move is searched with the full window; the rest are probed with a
    /// null window and only re-searched when they surprise us by beating alpha.
    fn search(
        board: &Board,
        alpha: i32,
        beta: i32,
        depth: usize,
        passed: bool,
        w: &PhaseWeights,
        tt: &mut SearchTt,
    ) -> i32 {
        debug_assert!(alpha <= beta);
        let (black_moves, parts) = board.get_valid_mask();
        let (white_moves, _) = board.switch().get_valid_mask();
        if depth == 0 || (black_moves == 0 && passed) {
            return Self::evaluate(board, &(black_moves, white_moves), w);
        }
        if black_moves == 0 {
            return -Self::search(&board.switch(), -beta, -alpha, depth, true, w, tt);
        }

        let key = (board.0, board.1);
        let mut alpha = alpha;
        let mut beta = beta;
        let orig_alpha = alpha;

        let mut tt_move = 0;
        if let Some(&e) = tt.get(&key) {
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

        let moves = Self::ordered_moves(board, black_moves, &parts, tt_move);
        let mut best = -INF;
        let mut best_move = moves[0];
        let mut first = true;
        for &mov in moves.iter() {
            let child = board.flip_with_hints(mov, &parts).switch();
            let score = if first {
                -Self::search(&child, -beta, -alpha, depth - 1, false, w, tt)
            } else {
                // Null-window probe; re-search on a fail-high inside the window.
                let s = -Self::search(&child, -alpha - 1, -alpha, depth - 1, false, w, tt);
                if s > alpha && s < beta {
                    -Self::search(&child, -beta, -alpha, depth - 1, false, w, tt)
                } else {
                    s
                }
            };
            first = false;
            if score > best {
                best = score;
                best_move = mov;
            }
            alpha = max(alpha, score);
            if alpha >= beta {
                break;
            }
        }

        let bound = if best <= orig_alpha {
            Bound::Upper
        } else if best >= beta {
            Bound::Lower
        } else {
            Bound::Exact
        };
        tt.insert(key, SearchEntry { depth: depth as u8, value: best, bound, best_move });
        best
    }

    /// Exact endgame solver, also using PVS. Unlike AB3's win/loss/draw-only
    /// solver this returns the exact final *disc difference* (my discs − opp
    /// discs), so it plays for the widest possible margin, not just any win.
    fn solve(board: &Board, alpha: i32, beta: i32, passed: bool, tt: &mut SolveTt) -> i32 {
        debug_assert!(alpha <= beta);
        let (my_moves, parts) = board.get_valid_mask();
        if my_moves == 0 {
            if passed {
                let (me, opp) = board.count();
                return me as i32 - opp as i32;
            }
            return -Self::solve(&board.switch(), -beta, -alpha, true, tt);
        }

        let key = (board.0, board.1);
        let mut alpha = alpha;
        let mut beta = beta;
        let orig_alpha = alpha;

        let mut tt_move = 0;
        if let Some(&e) = tt.get(&key) {
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

        let moves = Self::ordered_moves(board, my_moves, &parts, tt_move);
        let mut best = -INF;
        let mut best_move = moves[0];
        let mut first = true;
        for &mov in moves.iter() {
            let child = board.flip_with_hints(mov, &parts).switch();
            let score = if first {
                -Self::solve(&child, -beta, -alpha, false, tt)
            } else {
                let s = -Self::solve(&child, -alpha - 1, -alpha, false, tt);
                if s > alpha && s < beta {
                    -Self::solve(&child, -beta, -alpha, false, tt)
                } else {
                    s
                }
            };
            first = false;
            if score > best {
                best = score;
                best_move = mov;
            }
            alpha = max(alpha, score);
            if alpha >= beta {
                break;
            }
        }

        let bound = if best <= orig_alpha {
            Bound::Upper
        } else if best >= beta {
            Bound::Lower
        } else {
            Bound::Exact
        };
        tt.insert(key, SolveEntry { value: best, bound, best_move });
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

    /// Counts frontier discs (discs adjacent to at least one empty cell) for
    /// black and white. Frontier discs are usually a liability.
    #[inline]
    fn frontier_counts(board: &Board) -> (u32, u32) {
        let Board(black, white) = *board;
        let empty = !(black | white);
        let neighbours = Self::neighbours(empty);
        ((black & neighbours).count_ones(), (white & neighbours).count_ones())
    }

    /// Potential mobility for black and white: the number of empty cells adjacent
    /// to the *opponent's* discs — squares where each side may gain future moves.
    /// A cheap, well-known complement to current mobility.
    #[inline]
    fn potential_mobility(board: &Board) -> (u32, u32) {
        let Board(black, white) = *board;
        let empty = !(black | white);
        let black_pmob = (Self::neighbours(white) & empty).count_ones();
        let white_pmob = (Self::neighbours(black) & empty).count_ones();
        (black_pmob, white_pmob)
    }

    /// Flood-fill stable-disc count for black and white. A disc is stable when it
    /// can never be flipped: along each of the four axes (horizontal, vertical,
    /// and the two diagonals) it is either on the board edge for that axis, part
    /// of a completely filled line, or flanked by an already-stable same-coloured
    /// disc. Seeded by the corners, iterated to a fixpoint. This is a much tighter
    /// estimate than the old corner-anchored edge-run lower bound.
    #[inline]
    fn stable_full(board: &Board) -> (u32, u32) {
        let Board(black, white) = *board;
        let empty = !(black | white);
        const ALL: Mask = u64::MAX;

        // Spread `bits` fully along one axis: `s` cells up (`<< s`, guarded by
        // `gu`) and down (`>> s`, guarded by `gd`), 7 steps covering the board.
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

        // Cells whose whole line (in each axis) is fully occupied = the complement
        // of cells sharing that line with an empty.
        let full_h = !spread(empty, 1, NOT_FILE_A, NOT_FILE_H);
        let full_v = !spread(empty, 8, ALL, ALL);
        let full_d = !spread(empty, 9, NOT_FILE_A, NOT_FILE_H);
        let full_a = !spread(empty, 7, NOT_FILE_H, NOT_FILE_A);

        const FILE_A: Mask = 0x0101010101010101;
        const FILE_H: Mask = 0x8080808080808080;
        const RANK_1: Mask = 0x00000000000000FF;
        const RANK_8: Mask = 0xFF00000000000000;
        const BORDER: Mask = FILE_A | FILE_H | RANK_1 | RANK_8;

        // Static per-axis safety: a full line, or the board edge for that axis.
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

        let discdiff = black.count_ones() as i32 - white.count_ones() as i32;

        wt.pos * posdiff
            + wt.mob * mobdiff
            + wt.pmob * pmobdiff
            + wt.front * frontdiff
            + wt.stab * stabdiff
            + wt.disc * discdiff
    }
}

impl Player for AlphaBeta4Player {
    fn next(&mut self, board: &Board) -> Option<Mask> {
        let (black_moves, parts) = board.get_valid_mask();
        if black_moves == 0 {
            return None;
        }

        // Root moves, shuffled once so ties are broken uniformly at random.
        let mut moves = (0..H * W)
            .map(|i| 1u64 << i)
            .filter(|&mov| mov & black_moves == mov)
            .collect::<Vec<_>>();
        let n = moves.len();
        for i in 0..n - 1 {
            moves.swap(i, i + self.rand.next() as usize % (n - i));
        }

        let (black, white) = board.count();
        let empties = (H * W) as u32 - black - white;

        // Endgame: one exact, disc-differential pass (no iterative deepening).
        if empties <= ENDGAME_EMPTIES {
            let mut solve_tt = SolveTt::default();
            let mut alpha = -INF;
            let mut best_position = moves[0];
            for &mov in moves.iter() {
                let reversed = board.flip_with_hints(mov, &parts);
                let score = -Self::solve(&reversed.switch(), -INF, -alpha, false, &mut solve_tt);
                if score > alpha {
                    alpha = score;
                    best_position = mov;
                }
            }
            return Some(best_position);
        }

        // Iterative deepening over odd depths, sharing one transposition table and
        // re-ordering the root moves best-first after each iteration. This is what
        // finally makes alpha-beta prune at the root (the previous best move sets a
        // strong alpha before the weaker moves are tried).
        let mut search_tt = SearchTt::default();
        let mut best_position = moves[0];
        let mut depth = 1;
        while depth <= SEARCH_DEPTH {
            let mut scored: Vec<(i32, Mask)> = Vec::with_capacity(moves.len());
            let mut alpha = -INF;
            for &mov in moves.iter() {
                let reversed = board.flip_with_hints(mov, &parts);
                let score = -Self::search(
                    &reversed.switch(),
                    -INF,
                    -alpha,
                    depth,
                    false,
                    &self.weights,
                    &mut search_tt,
                );
                if score > alpha {
                    alpha = score;
                }
                scored.push((score, mov));
            }
            // Best-first ordering for the next, deeper iteration.
            scored.sort_by(|a, b| b.0.cmp(&a.0));
            best_position = scored[0].1;
            moves = scored.into_iter().map(|(_, mov)| mov).collect();
            depth += 2;
        }
        Some(best_position)
    }

    fn name(&self) -> &'static str {
        "Alpha-Beta4"
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
        let (b, w) = AlphaBeta4Player::frontier_counts(&Board::new());
        assert_eq!((b, w), (2, 2));
    }

    #[test]
    fn potential_mobility_initial_board_symmetric() {
        // The initial position is symmetric under a 180° rotation that swaps
        // colours, so both sides must have equal (and non-zero) potential mobility.
        let (b, w) = AlphaBeta4Player::potential_mobility(&Board::new());
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
        assert_eq!(AlphaBeta4Player::stable_full(&board), (4, 0));
    }

    #[test]
    fn stable_full_full_edge_from_corner() {
        let mut top = 0u64;
        for c in 0..8 {
            top |= position_to_mask(0, c);
        }
        let board = Board(top, 0);
        assert_eq!(AlphaBeta4Player::stable_full(&board), (8, 0));
    }

    #[test]
    fn stable_full_full_board_all_stable() {
        // A completely filled board: every disc is stable (no empty to flip into).
        let board = Board(u64::MAX, 0);
        assert_eq!(AlphaBeta4Player::stable_full(&board), (64, 0));
    }

    // --- Health gate: AlphaBeta4 must crush the random player. ---
    // Relative strength vs AlphaBeta3 is measured in `benches/duel.rs`; this is
    // only a fast correctness gate.

    const GAMES: u32 = 20;
    const MIN_WINS: u32 = 20;

    fn play_game(seed: u32, ab_is_black: bool) -> Result<(), (u32, Winner, (u32, u32))> {
        let ab = || Box::new(AlphaBeta4Player::new(seed));
        let rand = || Box::new(RandomPlayer::new(seed.wrapping_add(1_000_000)));
        let (expected, mut gm) = if ab_is_black {
            (Winner::Black, GameManager::new(ab(), rand()))
        } else {
            (Winner::White, GameManager::new(rand(), ab()))
        };
        gm.verbose = false;
        gm.playout();
        let result = gm.result.as_ref().expect("game must be finished");
        if std::mem::discriminant(&result.winner) == std::mem::discriminant(&expected) {
            Ok(())
        } else {
            Err((seed, result.winner.clone(), result.disks))
        }
    }

    fn assert_dominates(ab_is_black: bool) {
        let threads = std::thread::available_parallelism()
            .map(|n| (n.get() - 1).max(1))
            .unwrap_or(1);
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
            "Alpha-Beta4 ({side}) won only {wins}/{GAMES} (need >= {MIN_WINS}); lost: {losses:?}"
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
