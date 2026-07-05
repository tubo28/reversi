use crate::reversi::bitboard::{Board, Mask};
use crate::reversi::player::alphabeta4::{AlphaBeta4Player, PhaseWeights};
use crate::reversi::player::Player;

/// `AlphaBeta4` with tuned evaluation weights (search and evaluation are
/// otherwise identical to AB4 — this is a thin wrapper over
/// `AlphaBeta4Player::with_weights`). Two sweeps produced the changes:
///  - `benches/tune4.rs` (vs AB4-default): potential mobility `pmob = 20/20/10`
///    (up from AB4's hand-seeded 15/10/3). Dropping `pmob` entirely was net -12,
///    confirming the term helps; `flat 20/20/10` won net +4 and held up at
///    net +16 over a 50-game `duel42`.
///  - `benches/tune42.rs` (vs the pmob-tuned champion): positional weight
///    `pos = 140/140/110` (up from 100/100/80) won net +10 (13-3); frontier and
///    the other axes were already optimal.
///
/// Kept as a separate `Player` so it can be measured head-to-head in
/// `benches/duel42.rs`.
pub struct AlphaBeta42Player {
    inner: AlphaBeta4Player,
}

/// AB4's default weights with the tuned `pmob` and `pos` values swapped in.
fn tuned_weights() -> PhaseWeights {
    let mut w = PhaseWeights::default();
    // tune4: potential mobility.
    w.opening.pmob = 20;
    w.midgame.pmob = 20;
    w.endgame.pmob = 10;
    // tune42: positional table weight.
    w.opening.pos = 140;
    w.midgame.pos = 140;
    w.endgame.pos = 110;
    w
}

impl AlphaBeta42Player {
    pub fn new(seed: u32) -> AlphaBeta42Player {
        AlphaBeta42Player { inner: AlphaBeta4Player::with_weights(seed, tuned_weights()) }
    }
}

impl Player for AlphaBeta42Player {
    fn next(&mut self, board: &Board) -> Option<Mask> {
        self.inner.next(board)
    }

    fn name(&self) -> &'static str {
        "Alpha-Beta4-2"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reversi::gm::{GameManager, Winner};
    use crate::reversi::player::random::RandomPlayer;

    // Health gate: like every engine, AB4-2 must crush the random player. Its
    // relative strength vs AB4 is measured in `benches/duel42.rs`.
    const GAMES: u32 = 20;
    const MIN_WINS: u32 = 20;

    fn play_game(seed: u32, ab_is_black: bool) -> Result<(), (u32, Winner, (u32, u32))> {
        let ab = || Box::new(AlphaBeta42Player::new(seed));
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
            "Alpha-Beta4-2 ({side}) won only {wins}/{GAMES} (need >= {MIN_WINS}); lost: {losses:?}"
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
