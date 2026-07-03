//! Round-robin strength league between the three search engines
//! (`AlphaBeta`, `AlphaBeta2`, `AlphaBeta3`). For every ordered pair the first
//! engine plays black and the second white for `GAMES` seeded games; running
//! both orderings gives each unordered pair `GAMES` games as first and `GAMES`
//! as second ("先後 GAMES 試合ずつ").
//!
//! This is where relative AI strength is measured (`cargo test` only checks that
//! each engine beats the random player).
//!
//! Run with: `cargo bench --bench league`
//! (declared with `harness = false`, so this is a plain `main`).

use std::time::Instant;

use reversi::reversi::gm::{GameManager, Winner};
use reversi::reversi::player::alphabeta::AlphaBetaSearchPlayer;
use reversi::reversi::player::alphabeta2::AlphaBeta2Player;
use reversi::reversi::player::alphabeta3::AlphaBeta3Player;
use reversi::reversi::player::Player;

// Games per ordered pair (so each unordered pair plays GAMES as first + GAMES as
// second = 2 * GAMES total).
const GAMES: u32 = 25;

// `fn` pointers (Send + Copy) so game specs move cheaply across worker threads.
type Factory = fn(u32) -> Box<dyn Player>;

fn mk_ab(seed: u32) -> Box<dyn Player> {
    Box::new(AlphaBetaSearchPlayer::new(seed))
}
fn mk_ab2(seed: u32) -> Box<dyn Player> {
    Box::new(AlphaBeta2Player::new(seed))
}
fn mk_ab3(seed: u32) -> Box<dyn Player> {
    Box::new(AlphaBeta3Player::new(seed))
}

const ENGINES: [(&str, Factory); 3] =
    [("Alpha-Beta", mk_ab), ("Alpha-Beta2", mk_ab2), ("Alpha-Beta3", mk_ab3)];

// One scheduled game: `black`/`white` are indices into ENGINES.
#[derive(Clone, Copy)]
struct Spec {
    black: usize,
    white: usize,
    seed: u32,
}

// Result of a game: which colour won (or a draw).
#[derive(Clone, Copy)]
enum Outcome {
    Black,
    White,
    Draw,
}

fn play(spec: Spec) -> Outcome {
    // Distinct seed streams so the two engines never share randomness.
    let black = ENGINES[spec.black].1(spec.seed);
    let white = ENGINES[spec.white].1(spec.seed.wrapping_add(1_000_000));
    let mut gm = GameManager::new(black, white);
    gm.verbose = false;
    gm.playout();
    match gm.result.as_ref().expect("game must be finished").winner {
        Winner::Black => Outcome::Black,
        Winner::White => Outcome::White,
        Winner::Draw => Outcome::Draw,
    }
}

fn main() {
    let n = ENGINES.len();

    // Build the schedule: every ordered pair, GAMES seeded games each.
    let mut specs = Vec::new();
    for black in 0..n {
        for white in 0..n {
            if black == white {
                continue;
            }
            for seed in 0..GAMES {
                specs.push(Spec { black, white, seed });
            }
        }
    }

    // Play in parallel across the available CPUs (leave one free).
    let threads = std::thread::available_parallelism().map(|c| (c.get() - 1).max(1)).unwrap_or(1);

    let start = Instant::now();
    let specs_ref = &specs;
    let per_thread: Vec<Vec<(Spec, Outcome)>> = std::thread::scope(|s| {
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

    // head[i][j] = (wins, losses, draws) of engine i vs engine j (both colours).
    let mut head = vec![vec![(0u32, 0u32, 0u32); n]; n];
    for (spec, outcome) in per_thread.into_iter().flatten() {
        let (b, w) = (spec.black, spec.white);
        match outcome {
            Outcome::Black => {
                head[b][w].0 += 1;
                head[w][b].1 += 1;
            }
            Outcome::White => {
                head[w][b].0 += 1;
                head[b][w].1 += 1;
            }
            Outcome::Draw => {
                head[b][w].2 += 1;
                head[w][b].2 += 1;
            }
        }
    }

    // Standings: total W/L/D and points (win = 1, draw = 0.5).
    let mut standings: Vec<(usize, u32, u32, u32, f64)> = (0..n)
        .map(|i| {
            let (mut w, mut l, mut d) = (0u32, 0u32, 0u32);
            for j in 0..n {
                if i == j {
                    continue;
                }
                w += head[i][j].0;
                l += head[i][j].1;
                d += head[i][j].2;
            }
            (i, w, l, d, w as f64 + 0.5 * d as f64)
        })
        .collect();
    standings.sort_by(|a, b| b.4.partial_cmp(&a.4).unwrap());

    let total_games = specs.len();
    println!("league: {} engines, {GAMES} games / ordered pair, {total_games} games total", n);
    println!("elapsed: {:.2}s ({} threads)", elapsed.as_secs_f64(), threads);
    println!();

    // Head-to-head matrix (row engine's W-L-D vs each column engine).
    print!("{:<14}", "");
    for (name, _) in ENGINES.iter() {
        print!("{:>14}", name);
    }
    println!();
    for i in 0..n {
        print!("{:<14}", ENGINES[i].0);
        for j in 0..n {
            if i == j {
                print!("{:>14}", "-");
            } else {
                let (w, l, d) = head[i][j];
                print!("{:>14}", format!("{w}-{l}-{d}"));
            }
        }
        println!();
    }
    println!();

    // Final standings.
    println!("{:<14} {:>4} {:>4} {:>4} {:>7}", "engine", "W", "L", "D", "pts");
    for (i, w, l, d, pts) in standings.iter() {
        println!("{:<14} {:>4} {:>4} {:>4} {:>7.1}", ENGINES[*i].0, w, l, d, pts);
    }
}
