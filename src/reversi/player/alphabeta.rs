use crate::reversi::bitboard::*;
use crate::reversi::player::*;
use crate::reversi::{H, W};
use std::cmp::min;
use std::collections::HashMap;

/// Player by alpha-beta search algorithm.
pub struct AlphaBetaSearchPlayer {
    rand: rand::Xor128,
}

const SEARCH_DEPTH: usize = 7;

// When this few empty cells remain, switch to an exact endgame solver
// (search to the end of the game) instead of the heuristic depth-limited search.
// WLD prunes strongly, so 10 is solved almost instantly; it can be raised to 12-14.
const ENDGAME_EMPTIES: u32 = 10;

// A enough large evaluate value.
const INF: i32 = 100_000_000;

// The four corner cells. Capturing a corner is almost always good, so moves that
// land on a corner are tried first during move ordering.
const CORNERS: Mask =
    0b_10000001_00000000_00000000_00000000_00000000_00000000_00000000_10000001;

/// Kind of value stored in a transposition table entry, w.r.t. the search window
/// it was produced with.
/// - `Exact`: the value is the true (pv) score.
/// - `Lower`: search failed high, so the true score is >= value (a lower bound).
/// - `Upper`: search failed low, so the true score is <= value (an upper bound).
#[derive(Clone, Copy)]
enum Bound {
    Exact,
    Lower,
    Upper,
}

/// Transposition table entry for the heuristic depth-limited search.
/// `depth` is the remaining search depth this value was computed with; a cached
/// value may only be reused for a lookup that needs an equal-or-shallower search.
#[derive(Clone, Copy)]
struct SearchEntry {
    depth: u8,
    value: i32,
    bound: Bound,
    best_move: Mask,
}

/// Transposition table entry for the exact endgame solver. No depth is needed:
/// `solve` always searches to the end of the game, so the value is fully accurate
/// for the whole subtree regardless of when it is looked up.
#[derive(Clone, Copy)]
struct SolveEntry {
    value: i32,
    bound: Bound,
    best_move: Mask,
}

// Boards are keyed by their raw (black, white) bitmasks. The board passed to the
// search always has the side-to-move as `.0`, so this pair fully identifies the
// position (whose turn it is included).
type SearchTt = HashMap<(Mask, Mask), SearchEntry>;
type SolveTt = HashMap<(Mask, Mask), SolveEntry>;

impl AlphaBetaSearchPlayer {
    pub fn new(seed: u32) -> AlphaBetaSearchPlayer {
        AlphaBetaSearchPlayer { rand: rand::Xor128::from_seed(seed) }
    }

    /// Orders the moves in `moves_mask` from most to least promising so that
    /// alpha-beta cuts off as early as possible. Good ordering is the single most
    /// important factor for pruning efficiency.
    ///
    /// Priority, highest first:
    ///  1. `tt_move` (the best move remembered for this position, if any),
    ///  2. moves that capture a corner,
    ///  3. moves that leave the opponent with fewer replies (low mobility).
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
                    // Always try the transposition-table move first.
                    i32::MAX
                } else {
                    // Look one ply ahead and prefer moves that give the opponent
                    // fewer legal replies (mobility restriction).
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
            // No valid moves, pass.
            return -Self::search(&board.switch(), -beta, -alpha, depth, true, tt);
        }

        let key = (board.0, board.1);
        let mut alpha = alpha;
        let mut beta = beta;
        let orig_alpha = alpha;

        // Probe the transposition table. A cached value computed at an equal-or-deeper
        // search can shrink the window (or return immediately). Its best move, if any,
        // is used to order the search even when the value itself is not usable.
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

        // Dive in next depth in best-first order with updating alpha.
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

        // Record what we learned so sibling and future searches can reuse it.
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

    /// Exact endgame solver. Searches to the end of the game (no depth limit) and
    /// returns the win/loss/draw result from the current player's perspective:
    /// +1 win, 0 draw, -1 loss. Used when only a few empty cells remain.
    fn solve(board: &Board, alpha: i32, beta: i32, passed: bool, tt: &mut SolveTt) -> i32 {
        debug_assert!(alpha <= beta);
        let (my_moves, parts) = board.get_valid_mask();
        if my_moves == 0 {
            if passed {
                // Both players passed -> game over. Score by raw disk counts,
                // matching evaluate() / GameManager::finalize().
                let (me, opp) = board.count();
                return (me as i32 - opp as i32).signum();
            }
            // No valid moves, pass.
            return -Self::solve(&board.switch(), -beta, -alpha, true, tt);
        }

        let key = (board.0, board.1);
        let mut alpha = alpha;
        let mut beta = beta;
        let orig_alpha = alpha;

        // Probe the endgame table. Since every entry is solved fully to the end,
        // the cached value is valid regardless of remaining depth; only the bound
        // (relative to the window it was produced with) has to be respected.
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

    /// A simple evaluate function. The higher value for the greater the advantage.
    #[inline]
    fn evaluate(board: &Board, moves: &(Mask, Mask)) -> i32 {
        let Board(black_disks, white_disks) = *board;
        let (black_moves, white_moves) = *moves;

        if white_moves == 0 && black_moves == 0 {
            // No valid moves, game over.
            if black_disks.count_ones() > white_disks.count_ones() {
                return INF;
            } else if black_disks.count_ones() < white_disks.count_ones() {
                return -INF;
            } else {
                return 0;
            }
        }

        #[inline]
        fn eval(disks: Mask, moves: Mask) -> i32 {
            // https://uguisu.skr.jp/othello/5-1.html
            // Disks on this range add 30 points.
            const ADD30: Mask =
                0b_10000001_00000000_00000000_00000000_00000000_00000000_00000000_10000001;
            // Sub 1 point.
            const SUB01: Mask =
                0b_00011000_00000000_00011000_10111101_10111101_00011000_00000000_00011000;
            // Sub 3 points
            const SUB03: Mask =
                0b_00000000_00111100_01000010_01000010_01000010_01000010_00111100_00000000;
            // Sub 12 points
            const SUB12: Mask =
                0b_01000010_10000001_00000000_00000000_00000000_00000000_10000001_01000010;
            // Sub 15 points
            const SUB16: Mask =
                0b_00000000_01000010_00000000_00000000_00000000_00000000_01000010_00000000;
            let mut weighted_disks = 0;
            weighted_disks += ((ADD30 & disks).count_ones() * 30) as i32;
            weighted_disks -= ((SUB01 & disks).count_ones() * 1) as i32;
            weighted_disks -= ((SUB03 & disks).count_ones() * 3) as i32;
            weighted_disks -= ((SUB12 & disks).count_ones() * 12) as i32;
            weighted_disks -= ((SUB16 & disks).count_ones() * 16) as i32;

            let num_moves = moves.count_ones() as i32;
            // add num_moves * 5 because it seems good when there is more valid positions.
            weighted_disks * 100 + num_moves * 5
        }
        return eval(black_disks, black_moves) - eval(white_disks, white_moves);
    }
}

impl Player for AlphaBetaSearchPlayer {
    fn next(&mut self, board: &Board) -> Option<Mask> {
        let (black_moves, parts) = board.get_valid_mask();
        if black_moves == 0 {
            None
        } else {
            // Enumerate all first moves and shuffle them so that, among moves that
            // tie for the best score, the first one encountered (and thus chosen) is
            // picked uniformly at random.
            let mut moves = (0..H * W)
                .map(|i| 1 << i)
                .filter(|&mov| mov & black_moves == mov)
                .collect::<Vec<_>>();
            let n = moves.len();
            // Do Fisher-Yates algorithm.
            for i in 0..n - 1 {
                moves.swap(i, i + self.rand.next() as usize % (n - i));
            }

            // Once only a few empty cells remain, solve the game exactly to the end
            // (WLD) instead of the heuristic depth-limited search.
            let (black, white) = board.count();
            let empties = (H * W) as u32 - black - white;
            let endgame = empties <= ENDGAME_EMPTIES;

            // Fresh transposition tables for this decision. Entries are keyed by the
            // exact board, so they stay valid for the whole move loop (sibling first
            // moves share many transposed positions) and are dropped afterwards.
            let mut search_tt = SearchTt::new();
            let mut solve_tt = SolveTt::new();

            // Search each first move, narrowing alpha with the best score so far to
            // prune clearly-worse moves. beta stays at INF since there is no upper
            // bound at the root. Only a strictly better score updates the choice, so
            // a worse move that fails high to exactly alpha can never be selected.
            let mut alpha = -INF;
            let mut best_position = moves[0];
            for &mov in moves.iter() {
                let revered = board.flip_with_hints(mov, &parts);
                let score = if endgame {
                    -Self::solve(&revered.switch(), -INF, -alpha, false, &mut solve_tt)
                } else {
                    -Self::search(&revered.switch(), -INF, -alpha, SEARCH_DEPTH, false, &mut search_tt)
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
        "Alpha-Beta"
    }
}

#[cfg(test)]
mod tests {
    use super::AlphaBetaSearchPlayer;
    use crate::reversi::gm::{GameManager, Winner};
    use crate::reversi::player::random::RandomPlayer;

    // Number of games played against the random player.
    const GAMES: u32 = 100;
    // The alpha-beta player is randomized and does not solve the endgame, so it may
    // lose to random on rare occasions. Require it to win the vast majority instead
    // of every single game. Seeds are fixed, so this test is fully deterministic.
    const MIN_WINS: u32 = 99;

    // Plays a single deterministic game between the alpha-beta player and the random
    // player. Returns Ok(()) if alpha-beta won, or Err with the loss details otherwise.
    fn play_game(seed: u32, alpha_beta_is_black: bool) -> Result<(), (u32, Winner, (u32, u32))> {
        // Use a distinct seed per game so the games differ, and different seeds
        // for the two players so they don't share a random stream.
        let ab = || Box::new(AlphaBetaSearchPlayer::new(seed));
        let rand = || Box::new(RandomPlayer::new(seed.wrapping_add(1_000_000)));
        // The side the alpha-beta player takes is the one we expect to win.
        let (expected, mut gm) = if alpha_beta_is_black {
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

    // Plays GAMES games between the alpha-beta player and the random player and
    // asserts the alpha-beta player wins at least MIN_WINS of them.
    // `alpha_beta_is_black` chooses whether the alpha-beta player moves first (black)
    // or second (white). Games are independent and each fully seeded, so they are run
    // in parallel across the available CPUs; the result is identical to running them
    // serially.
    fn assert_alpha_beta_dominates(alpha_beta_is_black: bool) {
        // Leave one CPU free (but always use at least one thread).
        let threads = std::thread::available_parallelism()
            .map(|n| (n.get() - 1).max(1))
            .unwrap_or(1);
        // Each thread handles a strided subset of the seeds (thread t: t, t+threads, ...).
        let per_thread: Vec<Vec<(u32, Winner, (u32, u32))>> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..threads)
                .map(|t| {
                    s.spawn(move || {
                        let mut losses = Vec::new();
                        let mut seed = t as u32;
                        while seed < GAMES {
                            if let Err(loss) = play_game(seed, alpha_beta_is_black) {
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
        let side = if alpha_beta_is_black { "black" } else { "white" };
        assert!(
            wins >= MIN_WINS,
            "alpha-beta ({side}) won only {wins}/{GAMES} (need >= {MIN_WINS}); lost games: {losses:?}"
        );
    }

    /// The alpha-beta player, playing black (first), must beat the random player in
    /// at least MIN_WINS of GAMES.
    #[test]
    fn beats_random_as_black_almost_always() {
        assert_alpha_beta_dominates(true);
    }

    /// The alpha-beta player, playing white (second), must beat the random player in
    /// at least MIN_WINS of GAMES.
    #[test]
    fn beats_random_as_white_almost_always() {
        assert_alpha_beta_dominates(false);
    }
}
