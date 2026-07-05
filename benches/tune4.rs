//! Potential-mobility (`pmob`) weight sweep for `AlphaBeta4`.
//!
//! AB4 introduced a `pmob` evaluation term seeded by hand (opening 15 / midgame
//! 10 / endgame 3, an untuned mirror of current mobility). This sweeps candidate
//! `pmob` weightings — all other terms fixed at the AB4 default — against the
//! fixed champion `AlphaBeta4` (default weights), ranking by net score
//! (wins - losses) with average disc margin as a lower-variance tiebreak.
//!
//! Includes a `no-pmob` (0/0/0) candidate so we can tell whether the term helps
//! at all, and `default` as a self-play control (should land near net 0).
//!
//! The winner becomes the default weighting of the `AlphaBeta4-2` variant.
//!
//! Run with: `cargo bench --bench tune4`  (harness = false).

use std::time::Instant;

use reversi::reversi::gm::{GameManager, Winner};
use reversi::reversi::player::alphabeta4::{AlphaBeta4Player, PhaseWeights};

// Games per colour per candidate (played as both black and white).
const GAMES: u32 = 8;

// Override just the per-phase `pmob` weights, keeping every other term at the
// AB4 default.
fn with_pmob(base: PhaseWeights, opening: i32, midgame: i32, endgame: i32) -> PhaseWeights {
    let mut p = base;
    p.opening.pmob = opening;
    p.midgame.pmob = midgame;
    p.endgame.pmob = endgame;
    p
}

fn candidates() -> Vec<(&'static str, PhaseWeights)> {
    let d = PhaseWeights::default();
    vec![
        ("default 15/10/3", d),
        ("no-pmob 0/0/0", with_pmob(d, 0, 0, 0)),
        ("low 8/5/2", with_pmob(d, 8, 5, 2)),
        ("high 30/20/6", with_pmob(d, 30, 20, 6)),
        ("flat 20/20/10", with_pmob(d, 20, 20, 10)),
        ("open-heavy 40/15/2", with_pmob(d, 40, 15, 2)),
        ("vhigh 50/35/10", with_pmob(d, 50, 35, 10)),
    ]
}

#[derive(Clone, Copy)]
struct Spec {
    cand: usize,
    seed: u32,
    cand_black: bool,
}

// Result of one game from the candidate's perspective: outcome and disc margin
// (candidate discs - champion discs).
struct GameOut {
    cand: usize,
    win: i32, // +1 win, -1 loss, 0 draw
    margin: i32,
}

fn play(spec: Spec, weights: PhaseWeights) -> GameOut {
    let cand = || Box::new(AlphaBeta4Player::with_weights(spec.seed, weights));
    let champ = || Box::new(AlphaBeta4Player::new(spec.seed.wrapping_add(1_000_000)));
    let (mut gm, cand_is_black) = if spec.cand_black {
        (GameManager::new(cand(), champ()), true)
    } else {
        (GameManager::new(champ(), cand()), false)
    };
    let res = gm.playout();
    let (black, white) = res.disks;
    let margin =
        if cand_is_black { black as i32 - white as i32 } else { white as i32 - black as i32 };
    let win = match res.winner {
        Winner::Black => {
            if cand_is_black {
                1
            } else {
                -1
            }
        }
        Winner::White => {
            if cand_is_black {
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

    // Every candidate plays GAMES games as black and GAMES as white vs champion.
    let mut specs = Vec::new();
    for (i, _) in cands.iter().enumerate() {
        for seed in 0..GAMES {
            specs.push(Spec { cand: i, seed, cand_black: true });
            specs.push(Spec { cand: i, seed, cand_black: false });
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
        "tune4: AlphaBeta4 pmob weights vs champion Alpha-Beta4, {} games each ({GAMES} per colour)",
        2 * GAMES
    );
    println!("elapsed: {:.1}s ({} threads)", elapsed.as_secs_f64(), threads);
    println!();
    println!(
        "{:<22} {:>4} {:>4} {:>4} {:>6} {:>10}",
        "candidate", "W", "L", "D", "net", "avg-marg"
    );
    for (i, win, loss, draw, net, avgm) in rows.iter() {
        println!(
            "{:<22} {:>4} {:>4} {:>4} {:>+6} {:>+10.2}",
            cands[*i].0, win, loss, draw, net, avgm
        );
    }
}
