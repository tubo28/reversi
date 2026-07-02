use crate::reversi::bitboard::*;
use crate::reversi::player::*;
use crate::reversi::{H, W};

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

impl AlphaBetaSearchPlayer {
    pub fn new(seed: u32) -> AlphaBetaSearchPlayer {
        AlphaBetaSearchPlayer { rand: rand::Xor128::from_seed(seed) }
    }

    fn search(&mut self, board: &Board, alpha: i32, beta: i32, depth: usize, passed: bool) -> i32 {
        debug_assert!(alpha <= beta);
        let (black_moves, parts) = board.get_valid_mask();
        let (white_moves, _) = board.switch().get_valid_mask();
        if depth == 0 || (black_moves == 0 && passed) {
            Self::evaluate(board, &(black_moves, white_moves))
        } else if black_moves == 0 {
            // No valid moves, pass.
            -self.search(&board.switch(), -beta, -alpha, depth, true)
        } else {
            let mut alpha = alpha;
            // Enumerate moves and shuffle them
            let mut moves = (0..H * W)
                .map(|i| 1 << i)
                .filter(|&mov| mov & black_moves == mov)
                .collect::<Vec<_>>();
            let n = moves.len();
            // Do Fisher-Yates algorithm.
            for i in 0..n - 1 {
                moves.swap(i, i + self.rand.next() as usize % (n - i));
            }

            // Dive in next depth in random order with updating alpha.
            for &mov in moves.iter() {
                let flipped = board.flip_with_hints(mov, &parts);
                let score = -self.search(&flipped.switch(), -beta, -alpha, depth - 1, false);
                alpha = max(alpha, score);
                if alpha >= beta {
                    break;
                }
            }
            alpha
        }
    }

    /// Exact endgame solver. Searches to the end of the game (no depth limit) and
    /// returns the win/loss/draw result from the current player's perspective:
    /// +1 win, 0 draw, -1 loss. Used when only a few empty cells remain.
    fn solve(&mut self, board: &Board, alpha: i32, beta: i32, passed: bool) -> i32 {
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
            return -self.solve(&board.switch(), -beta, -alpha, true);
        }
        let mut alpha = alpha;
        // Natural bit order is fine here: move ordering only affects pruning speed,
        // not the WLD result. The randomized choice among equally-good moves is done
        // at the root in next().
        for mov in (0..H * W).map(|i| 1 << i).filter(|&m| m & my_moves == m) {
            let flipped = board.flip_with_hints(mov, &parts);
            let score = -self.solve(&flipped.switch(), -beta, -alpha, false);
            alpha = max(alpha, score);
            if alpha >= beta {
                break;
            }
        }
        alpha
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

            // Search each first move, narrowing alpha with the best score so far to
            // prune clearly-worse moves. beta stays at INF since there is no upper
            // bound at the root. Only a strictly better score updates the choice, so
            // a worse move that fails high to exactly alpha can never be selected.
            let mut alpha = -INF;
            let mut best_position = moves[0];
            for &mov in moves.iter() {
                let revered = board.flip_with_hints(mov, &parts);
                let score = if endgame {
                    -self.solve(&revered.switch(), -INF, -alpha, false)
                } else {
                    -self.search(&revered.switch(), -INF, -alpha, SEARCH_DEPTH, false)
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
    const MIN_WINS: u32 = 95;

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
