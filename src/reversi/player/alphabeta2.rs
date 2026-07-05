use crate::reversi::bitboard::*;
use crate::reversi::player::*;
use crate::reversi::{H, W};
use std::cmp::min;
use std::collections::HashMap;

/// Player by alpha-beta search with a stronger heuristic evaluation than
/// `AlphaBetaSearchPlayer`. The search machinery (alpha-beta + transposition
/// table + move ordering + exact endgame solver) is intentionally a copy of the
/// baseline player so the two can be compared head-to-head; only `evaluate`
/// differs. It adds three classic Othello signals on top of the positional
/// table + mobility: frontier discs (bad), stable discs (good), and
/// phase-dependent weights (opening / midgame / endgame emphasise different
/// things).
pub struct AlphaBeta2Player {
    rand: rand::Xor128,
}

const SEARCH_DEPTH: usize = 7;

// When this few empty cells remain, switch to an exact endgame solver.
const ENDGAME_EMPTIES: u32 = 10;

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

type SearchTt = HashMap<(Mask, Mask), SearchEntry>;
type SolveTt = HashMap<(Mask, Mask), SolveEntry>;

/// Linear-combination weights for the evaluation terms. Different game phases
/// use different weights. These are hand-picked starting values; they are meant
/// to be tuned by the `AlphaBeta` vs `AlphaBeta2` head-to-head match at the
/// bottom of this file (adjust, re-run, keep what wins).
struct Weights {
    pos: i32,
    mob: i32,
    front: i32,
    stab: i32,
    disc: i32,
}

impl AlphaBeta2Player {
    pub fn new(seed: u32) -> AlphaBeta2Player {
        AlphaBeta2Player { rand: rand::Xor128::from_seed(seed) }
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

    fn search(
        board: &Board,
        alpha: i32,
        beta: i32,
        depth: usize,
        passed: bool,
        tt: &mut SearchTt,
    ) -> i32 {
        debug_assert!(alpha <= beta);
        let (black_moves, parts) = board.get_valid_mask();
        let (white_moves, _) = board.switch().get_valid_mask();
        if depth == 0 || (black_moves == 0 && passed) {
            return Self::evaluate(board, &(black_moves, white_moves));
        }
        if black_moves == 0 {
            return -Self::search(&board.switch(), -beta, -alpha, depth, true, tt);
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
        for &mov in moves.iter() {
            let flipped = board.flip_with_hints(mov, &parts);
            let score = -Self::search(&flipped.switch(), -beta, -alpha, depth - 1, false, tt);
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

    /// Exact endgame solver (WLD). Identical to the baseline player.
    fn solve(board: &Board, alpha: i32, beta: i32, passed: bool, tt: &mut SolveTt) -> i32 {
        debug_assert!(alpha <= beta);
        let (my_moves, parts) = board.get_valid_mask();
        if my_moves == 0 {
            if passed {
                let (me, opp) = board.count();
                return (me as i32 - opp as i32).signum();
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
        for &mov in moves.iter() {
            let flipped = board.flip_with_hints(mov, &parts);
            let score = -Self::solve(&flipped.switch(), -beta, -alpha, false, tt);
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
    /// baseline player (corners good, X/C squares bad). Serves as a rough
    /// stability proxy; the explicit stability term below refines it.
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

    /// Counts frontier discs (discs adjacent to at least one empty cell) for
    /// black and white. Frontier discs are usually a liability: they can be
    /// captured, and having many of them tends to hand the opponent mobility.
    #[inline]
    fn frontier_counts(board: &Board) -> (u32, u32) {
        let Board(black, white) = *board;
        let empty = !(black | white);
        // Cells that neighbour an empty cell, in all 8 directions. Horizontal
        // components are masked so a shift cannot wrap across a row edge.
        let neighbours = ((empty << 1) & NOT_FILE_A)
            | ((empty >> 1) & NOT_FILE_H)
            | (empty << 8)
            | (empty >> 8)
            | ((empty << 9) & NOT_FILE_A)
            | ((empty >> 9) & NOT_FILE_H)
            | ((empty << 7) & NOT_FILE_H)
            | ((empty >> 7) & NOT_FILE_A);
        ((black & neighbours).count_ones(), (white & neighbours).count_ones())
    }

    /// Conservative (lower-bound) count of stable discs for black and white.
    /// A disc is counted as stable if it is a corner, or lies on an edge in an
    /// unbroken run of same-coloured discs anchored at a corner that colour
    /// owns. Such discs can never be flipped. This underestimates stability
    /// (it ignores interior stability) but is cheap and never over-claims.
    /// It could later be upgraded to a full 3^8 edge table.
    #[inline]
    fn stable_counts(board: &Board) -> (u32, u32) {
        let Board(black, white) = *board;

        // The four edges, each as its 8 cell masks ordered from one corner to
        // the other.
        let mut edges: [[Mask; 8]; 4] = [[0; 8]; 4];
        for i in 0..8u32 {
            edges[0][i as usize] = 1 << i; // top row
            edges[1][i as usize] = 1 << (56 + i); // bottom row
            edges[2][i as usize] = 1 << (i * 8); // left column
            edges[3][i as usize] = 1 << (i * 8 + 7); // right column
        }

        // Stable cells of `color` on one edge: same-coloured runs growing inward
        // from whichever ends are corners owned by `color`.
        fn edge_stable(cells: &[Mask; 8], color: Mask) -> Mask {
            let mut stable = 0;
            if cells[0] & color != 0 {
                for &c in cells.iter() {
                    if c & color != 0 {
                        stable |= c;
                    } else {
                        break;
                    }
                }
            }
            if cells[7] & color != 0 {
                for &c in cells.iter().rev() {
                    if c & color != 0 {
                        stable |= c;
                    } else {
                        break;
                    }
                }
            }
            stable
        }

        // Union the per-edge stable masks so corners shared by two edges are not
        // double-counted, then pop-count.
        let mut sb = 0u64;
        let mut sw = 0u64;
        for edge in &edges {
            sb |= edge_stable(edge, black);
            sw |= edge_stable(edge, white);
        }
        (sb.count_ones(), sw.count_ones())
    }

    /// Per-phase evaluation weights, selected by the number of empty cells.
    /// Starting values only - tune via the head-to-head match.
    #[inline]
    fn weights(empties: u32) -> Weights {
        if empties >= 40 {
            // Opening: mobility and frontier dominate; raw disc count is
            // irrelevant (leading on discs early is usually bad).
            Weights { pos: 100, mob: 20, front: 35, stab: 25, disc: 0 }
        } else if empties >= 20 {
            // Midgame: positional table and stability lead, mobility still matters.
            Weights { pos: 100, mob: 15, front: 25, stab: 45, disc: 0 }
        } else {
            // Late midgame heading into the endgame: stability and disc count
            // dominate, mobility fades.
            Weights { pos: 80, mob: 5, front: 10, stab: 70, disc: 12 }
        }
    }

    /// Enhanced evaluation. Higher is better for black. Combines the positional
    /// table, mobility, frontier discs, stable discs and (late) disc count with
    /// phase-dependent weights.
    #[inline]
    fn evaluate(board: &Board, moves: &(Mask, Mask)) -> i32 {
        let Board(black, white) = *board;
        let (black_moves, white_moves) = *moves;

        if black_moves == 0 && white_moves == 0 {
            // Game over: decide by final disk count.
            let (b, w) = (black.count_ones(), white.count_ones());
            return match b.cmp(&w) {
                std::cmp::Ordering::Greater => INF,
                std::cmp::Ordering::Less => -INF,
                std::cmp::Ordering::Equal => 0,
            };
        }

        let empties = 64 - (black | white).count_ones();
        let wt = Self::weights(empties);

        let posdiff = Self::positional(black) - Self::positional(white);
        let mobdiff = black_moves.count_ones() as i32 - white_moves.count_ones() as i32;

        let (bf, wf) = Self::frontier_counts(board);
        // Fewer frontier discs is better, so subtract our own.
        let frontdiff = wf as i32 - bf as i32;

        let (bs, ws) = Self::stable_counts(board);
        let stabdiff = bs as i32 - ws as i32;

        let discdiff = black.count_ones() as i32 - white.count_ones() as i32;

        wt.pos * posdiff
            + wt.mob * mobdiff
            + wt.front * frontdiff
            + wt.stab * stabdiff
            + wt.disc * discdiff
    }
}

impl Player for AlphaBeta2Player {
    fn next(&mut self, board: &Board) -> Option<Mask> {
        let (black_moves, parts) = board.get_valid_mask();
        if black_moves == 0 {
            None
        } else {
            // Shuffle first moves so ties are broken uniformly at random.
            let mut moves = (0..H * W)
                .map(|i| 1 << i)
                .filter(|&mov| mov & black_moves == mov)
                .collect::<Vec<_>>();
            let n = moves.len();
            for i in 0..n - 1 {
                moves.swap(i, i + self.rand.next() as usize % (n - i));
            }

            let (black, white) = board.count();
            let empties = (H * W) as u32 - black - white;
            let endgame = empties <= ENDGAME_EMPTIES;

            let mut search_tt = SearchTt::new();
            let mut solve_tt = SolveTt::new();

            let mut alpha = -INF;
            let mut best_position = moves[0];
            for &mov in moves.iter() {
                let reversed = board.flip_with_hints(mov, &parts);
                let score = if endgame {
                    -Self::solve(&reversed.switch(), -INF, -alpha, false, &mut solve_tt)
                } else {
                    -Self::search(
                        &reversed.switch(),
                        -INF,
                        -alpha,
                        SEARCH_DEPTH,
                        false,
                        &mut search_tt,
                    )
                };
                if score > alpha {
                    alpha = score;
                    best_position = mov;
                }
            }
            Some(best_position)
        }
    }

    fn name(&self) -> &'static str {
        "Alpha-Beta2"
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
        // On the opening position the four central discs each touch an empty
        // cell, so both sides have two frontier discs.
        let (b, w) = AlphaBeta2Player::frontier_counts(&Board::new());
        assert_eq!((b, w), (2, 2));
    }

    #[test]
    fn stable_counts_detect_corners_only() {
        // Black holds the four corners, nothing else -> exactly four stable
        // discs, all black; white has none.
        let corners = position_to_mask(0, 0)
            | position_to_mask(0, 7)
            | position_to_mask(7, 0)
            | position_to_mask(7, 7);
        let board = Board(corners, 0);
        assert_eq!(AlphaBeta2Player::stable_counts(&board), (4, 0));
    }

    #[test]
    fn stable_counts_full_edge_from_corner() {
        // A complete top edge owned by black is fully stable (8 discs); the
        // opposite colour has none.
        let mut top = 0u64;
        for c in 0..8 {
            top |= position_to_mask(0, c);
        }
        let board = Board(top, 0);
        assert_eq!(AlphaBeta2Player::stable_counts(&board), (8, 0));
    }

    // --- Health gate: AlphaBeta2 must crush the random player. ---
    // Relative strength against the other engines is measured in `benches/league.rs`,
    // not here; `cargo test` only checks correctness / that it beats Random.

    const GAMES: u32 = 100;
    const MIN_WINS: u32 = 99;

    fn play_game(seed: u32, ab_is_black: bool) -> Result<(), (u32, Winner, (u32, u32))> {
        let ab = || Box::new(AlphaBeta2Player::new(seed));
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
            "Alpha-Beta2 ({side}) won only {wins}/{GAMES} (need >= {MIN_WINS}); lost: {losses:?}"
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
