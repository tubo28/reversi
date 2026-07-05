//! Second-stage weight sweep for the tuned champion `AlphaBeta4-2`.
//!
//! `tune4` fixed the potential-mobility term (`pmob = 20/20/10`). This sweep
//! holds `pmob` there and varies the *other* evaluation levers (stability,
//! mobility, frontier, positional, disc) one axis at a time, playing each
//! candidate against the current champion (the `pmob`-tuned weights) and ranking
//! by net score (wins - losses) with average disc margin as a tiebreak.
//!
//! `champion (control)` is self-play and should land near net 0. Any candidate
//! that clearly beats it is a real improvement worth folding into AB4-2.
//!
//! Run with: `cargo bench --bench tune42`  (harness = false).

use std::time::Instant;

use reversi::reversi::gm::{GameManager, Winner};
use reversi::reversi::player::alphabeta4::{AlphaBeta4Player, PhaseWeights};

// Games per colour per candidate (played as both black and white).
const GAMES: u32 = 8;

// The current champion: AB4 default with the tune4-winning pmob (20/20/10).
fn champion() -> PhaseWeights {
    let mut w = PhaseWeights::default();
    w.opening.pmob = 20;
    w.midgame.pmob = 20;
    w.endgame.pmob = 10;
    w
}

fn tweak(mut w: PhaseWeights, f: impl Fn(&mut PhaseWeights)) -> PhaseWeights {
    f(&mut w);
    w
}

fn candidates() -> Vec<(&'static str, PhaseWeights)> {
    let base = champion();
    vec![
        ("champion (control)", base),
        (
            "more-stability",
            tweak(base, |w| {
                w.opening.stab = 55;
                w.midgame.stab = 90;
                w.endgame.stab = 130;
            }),
        ),
        (
            "less-stability",
            tweak(base, |w| {
                w.opening.stab = 28;
                w.midgame.stab = 50;
                w.endgame.stab = 75;
            }),
        ),
        (
            "more-mobility",
            tweak(base, |w| {
                w.opening.mob = 35;
                w.midgame.mob = 25;
                w.endgame.mob = 10;
            }),
        ),
        (
            "less-frontier",
            tweak(base, |w| {
                w.opening.front = 18;
                w.midgame.front = 12;
                w.endgame.front = 5;
            }),
        ),
        (
            "more-frontier",
            tweak(base, |w| {
                w.opening.front = 55;
                w.midgame.front = 40;
                w.endgame.front = 18;
            }),
        ),
        (
            "more-positional",
            tweak(base, |w| {
                w.opening.pos = 140;
                w.midgame.pos = 140;
                w.endgame.pos = 110;
            }),
        ),
        (
            "disc-earlier",
            tweak(base, |w| {
                w.midgame.disc = 6;
                w.endgame.disc = 24;
            }),
        ),
    ]
}

#[derive(Clone, Copy)]
struct Spec {
    cand: usize,
    seed: u32,
    cand_black: bool,
}

// One game from the candidate's perspective.
struct GameOut {
    cand: usize,
    win: i32, // +1 win, -1 loss, 0 draw
    margin: i32,
}

fn play(spec: Spec, weights: PhaseWeights) -> GameOut {
    let cand = || Box::new(AlphaBeta4Player::with_weights(spec.seed, weights));
    let champ =
        || Box::new(AlphaBeta4Player::with_weights(spec.seed.wrapping_add(1_000_000), champion()));
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
    rows.sort_by(|a, b| b.4.cmp(&a.4).then(b.5.partial_cmp(&a.5).unwrap()));

    println!(
        "tune42: AB4-2 non-pmob weights vs champion (pmob-tuned), {} games each ({GAMES} per colour)",
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
