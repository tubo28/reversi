//! Head-to-head strength gate for `AlphaBeta4` vs the current champion
//! `AlphaBeta3`: AB4 plays `GAMES` games as black and `GAMES` as white
//! (先後 GAMES 戦ずつ), seeded and in parallel. Prints AB4's W-L-D record and
//! its average final disc margin.
//!
//! Run with: `cargo bench --bench duel`  (declared with `harness = false`).

use std::time::Instant;

use reversi::reversi::gm::{GameManager, Winner};
use reversi::reversi::player::alphabeta3::AlphaBeta3Player;
use reversi::reversi::player::alphabeta4::AlphaBeta4Player;

// Games per colour for the challenger (played as both black and white).
const GAMES: u32 = 25;

#[derive(Clone, Copy)]
struct Spec {
    seed: u32,
    ab4_black: bool,
}

// One game from AlphaBeta4's perspective.
struct GameOut {
    win: i32, // +1 win, -1 loss, 0 draw
    margin: i32,
}

fn play(spec: Spec) -> GameOut {
    // Distinct seed streams so the two engines never share randomness.
    let ab4 = || Box::new(AlphaBeta4Player::new(spec.seed));
    let ab3 = || Box::new(AlphaBeta3Player::new(spec.seed.wrapping_add(1_000_000)));
    let (mut gm, ab4_is_black) = if spec.ab4_black {
        (GameManager::new(ab4(), ab3()), true)
    } else {
        (GameManager::new(ab3(), ab4()), false)
    };
    gm.verbose = false;
    gm.playout();
    let res = gm.result.as_ref().expect("game must be finished");
    let (black, white) = res.disks;
    let margin =
        if ab4_is_black { black as i32 - white as i32 } else { white as i32 - black as i32 };
    let win = match res.winner {
        Winner::Black => {
            if ab4_is_black {
                1
            } else {
                -1
            }
        }
        Winner::White => {
            if ab4_is_black {
                -1
            } else {
                1
            }
        }
        Winner::Draw => 0,
    };
    GameOut { win, margin }
}

fn main() {
    // AB4 plays GAMES games as black and GAMES as white.
    let mut specs = Vec::new();
    for seed in 0..GAMES {
        specs.push(Spec { seed, ab4_black: true });
        specs.push(Spec { seed, ab4_black: false });
    }

    let threads = std::thread::available_parallelism().map(|c| (c.get() - 1).max(1)).unwrap_or(1);

    let start = Instant::now();
    let specs_ref = &specs;
    let per_thread: Vec<Vec<(Spec, GameOut)>> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                s.spawn(move || {
                    let mut out = Vec::new();
                    let mut i = t;
                    while i < specs_ref.len() {
                        let spec = specs_ref[i];
                        out.push((spec, play(spec)));
                        i += threads;
                    }
                    out
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let elapsed = start.elapsed();

    // Aggregate overall and split by colour so first-player bias is visible.
    let (mut w, mut l, mut d, mut margin_sum) = (0i32, 0i32, 0i32, 0i64);
    let (mut bw, mut bl, mut bd) = (0i32, 0i32, 0i32); // AB4 as black
    let (mut ww, mut wl, mut wd) = (0i32, 0i32, 0i32); // AB4 as white
    for (spec, g) in per_thread.into_iter().flatten() {
        margin_sum += g.margin as i64;
        let (tw, tl, td) = if spec.ab4_black {
            (&mut bw, &mut bl, &mut bd)
        } else {
            (&mut ww, &mut wl, &mut wd)
        };
        match g.win {
            1 => {
                w += 1;
                *tw += 1;
            }
            -1 => {
                l += 1;
                *tl += 1;
            }
            _ => {
                d += 1;
                *td += 1;
            }
        }
    }

    let total = specs.len() as i64;
    println!("duel: Alpha-Beta4 (challenger) vs Alpha-Beta3, {GAMES} games / colour, {total} total");
    println!("elapsed: {:.2}s ({} threads)", elapsed.as_secs_f64(), threads);
    println!();
    println!("AB4 overall:      {w}-{l}-{d}  (W-L-D)   net {:+}", w - l);
    println!("AB4 as black:     {bw}-{bl}-{bd}");
    println!("AB4 as white:     {ww}-{wl}-{wd}");
    println!("AB4 avg margin:   {:+.2} discs", margin_sum as f64 / total as f64);
    println!();
    if w > l {
        println!("=> AB4 beats AB3 ({w} > {l}).");
    } else if w < l {
        println!("=> AB4 is WORSE than AB3 ({w} < {l}) — revert/adjust.");
    } else {
        println!("=> AB4 ties AB3 ({w} = {l}).");
    }
}
