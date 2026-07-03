//! Head-to-head confirmation for the fully-tuned `AlphaBeta4-2` (potential
//! mobility from `tune4` + positional from `tune42`) against the intermediate
//! champion it was last measured against — AB4 with only the `pmob` tune applied.
//! This isolates the `tune42` positional gain over a larger 50-game sample than
//! the tuning sweep itself. AB4-2 plays `GAMES` games as black and `GAMES` as
//! white (先後 GAMES 戦ずつ), seeded and in parallel.
//!
//! Run with: `cargo bench --bench duel42`  (declared with `harness = false`).

use std::time::Instant;

use reversi::reversi::player::alphabeta4::{AlphaBeta4Player, PhaseWeights};
use reversi::reversi::player::alphabeta42::AlphaBeta42Player;
use reversi::reversi::gm::{GameManager, Winner};

// Games per colour for the challenger (played as both black and white).
const GAMES: u32 = 25;

// The intermediate champion: AB4 with only the tune4 pmob change (20/20/10),
// i.e. AB4-2 *before* the tune42 positional bump. The current AB4-2 is measured
// against this to isolate the positional gain.
fn pmob_only() -> PhaseWeights {
    let mut w = PhaseWeights::default();
    w.opening.pmob = 20;
    w.midgame.pmob = 20;
    w.endgame.pmob = 10;
    w
}

#[derive(Clone, Copy)]
struct Spec {
    seed: u32,
    chal_black: bool,
}

// One game from the challenger (AB4-2)'s perspective.
struct GameOut {
    win: i32, // +1 win, -1 loss, 0 draw
    margin: i32,
}

fn play(spec: Spec) -> GameOut {
    // Distinct seed streams so the two engines never share randomness.
    let chal = || Box::new(AlphaBeta42Player::new(spec.seed));
    let champ =
        || Box::new(AlphaBeta4Player::with_weights(spec.seed.wrapping_add(1_000_000), pmob_only()));
    let (mut gm, chal_is_black) = if spec.chal_black {
        (GameManager::new(chal(), champ()), true)
    } else {
        (GameManager::new(champ(), chal()), false)
    };
    gm.verbose = false;
    gm.playout();
    let res = gm.result.as_ref().expect("game must be finished");
    let (black, white) = res.disks;
    let margin =
        if chal_is_black { black as i32 - white as i32 } else { white as i32 - black as i32 };
    let win = match res.winner {
        Winner::Black => {
            if chal_is_black {
                1
            } else {
                -1
            }
        }
        Winner::White => {
            if chal_is_black {
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
    // AB4-2 plays GAMES games as black and GAMES as white.
    let mut specs = Vec::new();
    for seed in 0..GAMES {
        specs.push(Spec { seed, chal_black: true });
        specs.push(Spec { seed, chal_black: false });
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
    let (mut bw, mut bl, mut bd) = (0i32, 0i32, 0i32); // AB4-2 as black
    let (mut ww, mut wl, mut wd) = (0i32, 0i32, 0i32); // AB4-2 as white
    for (spec, g) in per_thread.into_iter().flatten() {
        margin_sum += g.margin as i64;
        let (tw, tl, td) = if spec.chal_black {
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
    println!(
        "duel42: Alpha-Beta4-2 (pmob+pos) vs pmob-only champion, {GAMES} games / colour, {total} total"
    );
    println!("elapsed: {:.2}s ({} threads)", elapsed.as_secs_f64(), threads);
    println!();
    println!("AB4-2 overall:    {w}-{l}-{d}  (W-L-D)   net {:+}", w - l);
    println!("AB4-2 as black:   {bw}-{bl}-{bd}");
    println!("AB4-2 as white:   {ww}-{wl}-{wd}");
    println!("AB4-2 avg margin: {:+.2} discs", margin_sum as f64 / total as f64);
    println!();
    if w > l {
        println!("=> the positional bump helps: AB4-2 beats the pmob-only champion ({w} > {l}).");
    } else if w < l {
        println!("=> the positional bump HURTS ({w} < {l}) — revert pos in AB4-2.");
    } else {
        println!("=> the positional bump is neutral ({w} = {l}).");
    }
}
