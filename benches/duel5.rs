//! Head-to-head gate for `AlphaBeta5` (node-budget iterative deepening,
//! aspiration windows, killer / history ordering, carried-over endgame solve
//! table) against the reigning champion `AlphaBeta4-2`. AB5 plays `GAMES` games
//! as black and `GAMES` as white from random openings (see `OPENING_PLIES`),
//! seeded and in parallel, with a fixed node budget matching the shipped default
//! so the strength reading matches the browser build.
//!
//! It also prints a calibration line: a self-play game measuring per-move nodes,
//! reached depth, and wall time, so `DEFAULT_NODE_BUDGET` can be set from real
//! numbers. (The budget is deterministic; the timing is only for calibration.)
//!
//! Run with: `cargo bench --bench duel5`  (declared with `harness = false`).

use std::time::Instant;

use reversi::reversi::bitboard::{legal_moves, Board, Mask};
use reversi::reversi::player::alphabeta42::AlphaBeta42Player;
use reversi::reversi::player::alphabeta5::AlphaBeta5Player;
use reversi::reversi::player::Player;
use reversi::reversi::rand::Xor128;

// Games per colour for the challenger (played as both black and white).
const GAMES: u32 = 50;

// Node budget per move for AB5 in the duel, matching the shipped
// `DEFAULT_NODE_BUDGET`. Measured in browser V8 this is ~0.5s avg / ~1.0s max per
// move (wasm is only ~1.5x slower than native here).
const DUEL_BUDGET: u64 = 4_000_000;

// Random opening plies played before the engines take over. Strong deep search
// has few scoring ties, so without varied openings all same-colour games from
// the fixed Othello start collapse to one repeated line; a random opening per
// seed decorrelates the games and gives a real strength signal.
const OPENING_PLIES: u32 = 8;

#[derive(Clone, Copy)]
struct Spec {
    seed: u32,
    chal_black: bool,
}

// One game from the challenger (AB5)'s perspective.
struct GameOut {
    win: i32, // +1 win, -1 loss, 0 draw
    margin: i32,
}

fn pick_random(mask: Mask, rng: &mut Xor128) -> Mask {
    let count = mask.count_ones();
    let mut k = rng.next() % count;
    let mut m = mask;
    loop {
        let bit = m & m.wrapping_neg();
        if k == 0 {
            return bit;
        }
        k -= 1;
        m &= m - 1;
    }
}

fn play(spec: Spec) -> GameOut {
    // Distinct seed streams so the two engines never share randomness.
    let mut chal = AlphaBeta5Player::with_budget(spec.seed, DUEL_BUDGET);
    let mut champ = AlphaBeta42Player::new(spec.seed.wrapping_add(1_000_000));
    let mut rng = Xor128::from_seed(spec.seed ^ 0x9E37_79B9);

    // Drive the game manually so the first `OPENING_PLIES` moves are random
    // (identical for both games of a seed pair), then the engines take over.
    let mut board = Board::new();
    let mut turn_black = true;
    let mut ply = 0u32;
    let mut passes = 0;
    loop {
        // The mover always sees the board from the "black to move" perspective.
        let view = if turn_black { board.clone() } else { board.switch() };
        let lm = legal_moves(view.0, view.1);
        if lm == 0 {
            passes += 1;
            if passes == 2 {
                break;
            }
            turn_black = !turn_black;
            continue;
        }
        passes = 0;

        let mov = if ply < OPENING_PLIES {
            pick_random(lm, &mut rng)
        } else if turn_black == spec.chal_black {
            chal.next(&view).expect("legal move exists")
        } else {
            champ.next(&view).expect("legal move exists")
        };

        board = if turn_black { board.flip(mov) } else { board.switch().flip(mov).switch() };
        turn_black = !turn_black;
        ply += 1;
    }

    let (black, white) = board.count();
    let (my, opp) = if spec.chal_black { (black, white) } else { (white, black) };
    let margin = my as i32 - opp as i32;
    let win = margin.signum();
    GameOut { win, margin }
}

/// Plays a single AB5-vs-AB5 self-game (no `GameManager`, so we can read the
/// concrete player's per-move diagnostics) and reports average nodes, reached
/// depth, and wall time per move for the current `DUEL_BUDGET`.
fn calibrate() {
    let mut board = Board::new();
    let mut black = AlphaBeta5Player::with_budget(1, DUEL_BUDGET);
    let mut white = AlphaBeta5Player::with_budget(2, DUEL_BUDGET);
    let mut turn_black = true;
    let mut passes = 0;

    let mut moves = 0u64;
    let mut node_sum = 0u64;
    let mut depth_sum = 0usize;
    let mut max_move_time = 0.0f64;

    let start = Instant::now();
    loop {
        // The mover always sees the board from the "black to move" perspective.
        let view = if turn_black { board.clone() } else { board.switch() };
        if legal_moves(view.0, view.1) == 0 {
            passes += 1;
            if passes == 2 {
                break;
            }
            turn_black = !turn_black;
            continue;
        }
        passes = 0;

        let t0 = Instant::now();
        let mov = {
            let p = if turn_black { &mut black } else { &mut white };
            let mv = p.next(&view);
            node_sum += p.last_nodes;
            depth_sum += p.last_depth;
            mv
        };
        let dt = t0.elapsed().as_secs_f64();
        if dt > max_move_time {
            max_move_time = dt;
        }
        moves += 1;

        if let Some(mv) = mov {
            board = if turn_black {
                board.flip(mv)
            } else {
                board.switch().flip(mv).switch()
            };
        }
        turn_black = !turn_black;
    }
    let elapsed = start.elapsed().as_secs_f64();

    println!("calibration (single AB5 self-game, budget = {DUEL_BUDGET} nodes/move):");
    println!("  moves:            {moves}");
    println!("  avg nodes/move:   {:.0}", node_sum as f64 / moves as f64);
    println!("  avg reached depth:{:.1}", depth_sum as f64 / moves as f64);
    println!("  avg time/move:    {:.3}s (native)", elapsed / moves as f64);
    println!("  max time/move:    {max_move_time:.3}s (native)");
    println!("  => wasm is ~2-3x slower; scale the budget so max time/move stays ~1s in-browser.");
    println!();
}

fn main() {
    calibrate();

    // AB5 plays GAMES games as black and GAMES as white.
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
    let (mut bw, mut bl, mut bd) = (0i32, 0i32, 0i32); // AB5 as black
    let (mut ww, mut wl, mut wd) = (0i32, 0i32, 0i32); // AB5 as white
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
    println!("duel5: Alpha-Beta5 vs Alpha-Beta4-2, {GAMES} games / colour, {total} total");
    println!("elapsed: {:.2}s ({} threads)", elapsed.as_secs_f64(), threads);
    println!();
    println!("AB5 overall:    {w}-{l}-{d}  (W-L-D)   net {:+}", w - l);
    println!("AB5 as black:   {bw}-{bl}-{bd}");
    println!("AB5 as white:   {ww}-{wl}-{wd}");
    println!("AB5 avg margin: {:+.2} discs", margin_sum as f64 / total as f64);
    println!();
    if w > l {
        println!("=> AB5 beats AB4-2 ({w} > {l}).");
    } else if w < l {
        println!("=> AB5 LOSES to AB4-2 ({w} < {l}).");
    } else {
        println!("=> AB5 is neutral vs AB4-2 ({w} = {l}).");
    }
}
