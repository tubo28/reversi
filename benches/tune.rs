//! Weight-tuning sweep for `AlphaBeta3Player`.
//!
//! AlphaBeta3 already dominates AlphaBeta2 (same evaluation, shallower), so the
//! engine left to beat is the crude baseline `AlphaBeta`. This sweeps a handful
//! of candidate `PhaseWeights` against that fixed opponent and ranks them by
//! score (wins - losses), with average final disc margin as a lower-variance
//! secondary signal.
//!
//! Run with: `cargo bench --bench tune`  (harness = false).

use std::time::Instant;

use reversi::reversi::gm::{GameManager, Winner};
use reversi::reversi::player::alphabeta::AlphaBetaSearchPlayer;
use reversi::reversi::player::alphabeta3::{AlphaBeta3Player, PhaseWeights, Weights};

// Games per colour per candidate (played as both black and white).
const GAMES: u32 = 12;

fn w(pos: i32, mob: i32, front: i32, stab: i32, disc: i32) -> Weights {
    Weights { pos, mob, front, stab, disc }
}

// The candidate weightings to try. First is the current default (control).
fn candidates() -> Vec<(&'static str, PhaseWeights)> {
    let default = PhaseWeights::default();
    vec![
        ("default", default),
        (
            "more-mobility",
            PhaseWeights {
                opening: w(100, 40, 35, 25, 0),
                midgame: w(100, 30, 25, 45, 0),
                endgame: w(80, 10, 10, 70, 12),
            },
        ),
        (
            "more-stability",
            PhaseWeights {
                opening: w(100, 20, 35, 40, 0),
                midgame: w(100, 15, 25, 70, 0),
                endgame: w(80, 5, 10, 100, 12),
            },
        ),
        (
            "less-frontier",
            PhaseWeights {
                opening: w(100, 20, 15, 25, 0),
                midgame: w(100, 15, 12, 45, 0),
                endgame: w(80, 5, 5, 70, 12),
            },
        ),
        (
            "more-positional",
            PhaseWeights {
                opening: w(140, 20, 35, 25, 0),
                midgame: w(140, 15, 25, 45, 0),
                endgame: w(110, 5, 10, 70, 12),
            },
        ),
        (
            "disc-earlier",
            PhaseWeights {
                opening: w(100, 20, 35, 25, 0),
                midgame: w(100, 15, 25, 45, 6),
                endgame: w(80, 5, 10, 70, 24),
            },
        ),
        (
            "stab+mob-lessfront",
            PhaseWeights {
                opening: w(100, 30, 18, 35, 0),
                midgame: w(100, 22, 12, 65, 0),
                endgame: w(80, 8, 5, 100, 12),
            },
        ),
        (
            "baseline-like",
            PhaseWeights {
                opening: w(120, 40, 10, 10, 0),
                midgame: w(120, 30, 10, 20, 0),
                endgame: w(80, 10, 5, 40, 20),
            },
        ),
    ]
}

#[derive(Clone, Copy)]
struct Spec {
    cand: usize,
    seed: u32,
    ab3_black: bool,
}

// Result of one game from AlphaBeta3's perspective: outcome and disc margin
// (ab3 discs - baseline discs).
struct GameOut {
    cand: usize,
    win: i32, // +1 win, -1 loss, 0 draw
    margin: i32,
}

fn play(spec: Spec, weights: PhaseWeights) -> GameOut {
    let ab3 = || Box::new(AlphaBeta3Player::with_weights(spec.seed, weights));
    let base = || Box::new(AlphaBetaSearchPlayer::new(spec.seed.wrapping_add(1_000_000)));
    let (mut gm, ab3_is_black) = if spec.ab3_black {
        (GameManager::new(ab3(), base()), true)
    } else {
        (GameManager::new(base(), ab3()), false)
    };
    let res = gm.playout();
    let (black, white) = res.disks;
    let margin =
        if ab3_is_black { black as i32 - white as i32 } else { white as i32 - black as i32 };
    let win = match res.winner {
        Winner::Black => {
            if ab3_is_black {
                1
            } else {
                -1
            }
        }
        Winner::White => {
            if ab3_is_black {
                -1
            } else {
                1
            }
        }
        Winner::Draw => 0,
    };
    GameOut { cand: spec.cand, win, margin }
}

fn main() {
    let cands = candidates();

    // Every candidate plays GAMES games as black and GAMES as white vs baseline.
    let mut specs = Vec::new();
    for (i, _) in cands.iter().enumerate() {
        for seed in 0..GAMES {
            specs.push(Spec { cand: i, seed, ab3_black: true });
            specs.push(Spec { cand: i, seed, ab3_black: false });
        }
    }

    let threads = std::thread::available_parallelism().map(|c| (c.get() - 1).max(1)).unwrap_or(1);

    let start = Instant::now();
    let cands_ref = &cands;
    let specs_ref = &specs;
    let per_thread: Vec<Vec<GameOut>> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                s.spawn(move || {
                    let mut out = Vec::new();
                    let mut i = t;
                    while i < specs_ref.len() {
                        let spec = specs_ref[i];
                        out.push(play(spec, cands_ref[spec.cand].1));
                        i += threads;
                    }
                    out
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let elapsed = start.elapsed();

    // Aggregate: (wins, losses, draws, margin_sum) per candidate.
    let mut agg = vec![(0i32, 0i32, 0i32, 0i64); cands.len()];
    for g in per_thread.into_iter().flatten() {
        let a = &mut agg[g.cand];
        match g.win {
            1 => a.0 += 1,
            -1 => a.1 += 1,
            _ => a.2 += 1,
        }
        a.3 += g.margin as i64;
    }

    let games_each = (2 * GAMES) as i64;
    let mut rows: Vec<(usize, i32, i32, i32, i32, f64)> = (0..cands.len())
        .map(|i| {
            let (win, loss, draw, margin) = agg[i];
            (i, win, loss, draw, win - loss, margin as f64 / games_each as f64)
        })
        .collect();
    // Rank by net score, then by average margin.
    rows.sort_by(|a, b| b.4.cmp(&a.4).then(b.5.partial_cmp(&a.5).unwrap()));

    println!(
        "tune: AlphaBeta3 weights vs baseline Alpha-Beta, {} games each ({GAMES} per colour)",
        2 * GAMES
    );
    println!("elapsed: {:.1}s ({} threads)", elapsed.as_secs_f64(), threads);
    println!();
    println!(
        "{:<20} {:>4} {:>4} {:>4} {:>6} {:>10}",
        "candidate", "W", "L", "D", "net", "avg-marg"
    );
    for (i, win, loss, draw, net, avgm) in rows.iter() {
        println!(
            "{:<20} {:>4} {:>4} {:>4} {:>+6} {:>+10.2}",
            cands[*i].0, win, loss, draw, net, avgm
        );
    }
}
