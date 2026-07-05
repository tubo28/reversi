//! Strength measurement between search engines. Two modes:
//!
//! * No args — round-robin league over the fast engines (`LEAGUE_ROSTER`). For
//!   every ordered pair the first engine plays black and the second white for
//!   `LEAGUE_GAMES` seeded games; both orderings give each unordered pair
//!   `LEAGUE_GAMES` games as first and `LEAGUE_GAMES` as second.
//!
//! * `--duel <a> <b>` — head-to-head gate between two named players (see
//!   `PLAYERS` for the names). `<a>` plays `DUEL_GAMES` games as black and
//!   `DUEL_GAMES` as white (先後 DUEL_GAMES 戦ずつ) from random openings, and its
//!   W-L-D record, colour split, and average final disc margin are reported.
//!
//! This is where relative AI strength is measured (`cargo test` only checks that
//! each engine beats the random player).
//!
//! Run with:
//!   `cargo bench --bench league`                 (round-robin league)
//!   `cargo bench --bench league -- --duel ab5 ab42`   (head-to-head duel)
//! (declared with `harness = false`, so this is a plain `main`).

use std::time::Instant;

use reversi::reversi::bitboard::{legal_moves, Board, Mask};
use reversi::reversi::gm::{GameManager, Winner};
use reversi::reversi::player::alphabeta::AlphaBetaSearchPlayer;
use reversi::reversi::player::alphabeta2::AlphaBeta2Player;
use reversi::reversi::player::alphabeta3::AlphaBeta3Player;
use reversi::reversi::player::alphabeta4::AlphaBeta4Player;
use reversi::reversi::player::alphabeta42::AlphaBeta42Player;
use reversi::reversi::player::alphabeta5::AlphaBeta5Player;
use reversi::reversi::player::best::BestAiPlayer;
use reversi::reversi::player::random::RandomPlayer;
use reversi::reversi::player::Player;
use reversi::reversi::rand::Xor128;

// Games per ordered pair in the round-robin league (so each unordered pair
// plays LEAGUE_GAMES as first + LEAGUE_GAMES as second = 2 * LEAGUE_GAMES total).
const LEAGUE_GAMES: u32 = 25;

// Games per colour for the challenger in a `--duel` (played as both black and
// white).
const DUEL_GAMES: u32 = 50;

// Random opening plies played before the engines take over in a `--duel`. Strong
// deep search has few scoring ties, so without varied openings all same-colour
// games from the fixed Othello start collapse to one repeated line; a random
// opening per seed decorrelates the games and gives a real strength signal.
const OPENING_PLIES: u32 = 8;

// `fn` pointers (Send + Copy) so game specs move cheaply across worker threads.
type Factory = fn(u32) -> Box<dyn Player>;

// Registry of every player addressable by name (for `--duel <a> <b>`).
const PLAYERS: &[(&str, Factory)] = &[
    ("random", |seed| Box::new(RandomPlayer::new(seed))),
    ("ab", |seed| Box::new(AlphaBetaSearchPlayer::new(seed))),
    ("ab2", |seed| Box::new(AlphaBeta2Player::new(seed))),
    ("ab3", |seed| Box::new(AlphaBeta3Player::new(seed))),
    ("ab4", |seed| Box::new(AlphaBeta4Player::new(seed))),
    ("ab42", |seed| Box::new(AlphaBeta42Player::new(seed))),
    ("ab5", |seed| Box::new(AlphaBeta5Player::new(seed))),
    ("best", |seed| Box::new(BestAiPlayer::new(seed))),
];

// The fast engines that play the round-robin league (names into PLAYERS).
const LEAGUE_ROSTER: &[&str] = &["ab", "ab2", "ab3", "ab42", "ab5"];

fn lookup(name: &str) -> (&'static str, Factory) {
    match PLAYERS.iter().find(|(n, _)| *n == name) {
        Some(&(n, f)) => (n, f),
        None => {
            let names: Vec<&str> = PLAYERS.iter().map(|(n, _)| *n).collect();
            eprintln!("unknown player '{name}'. known players: {}", names.join(", "));
            std::process::exit(2);
        }
    }
}

fn worker_threads() -> usize {
    std::thread::available_parallelism().map(|c| (c.get() - 1).max(1)).unwrap_or(1)
}

// Play `specs` in parallel across the available CPUs (leave one free), returning
// each spec paired with its result. Order is not preserved (irrelevant here,
// since callers aggregate).
fn run_parallel<S, R>(specs: &[S], play: impl Fn(S) -> R + Send + Copy) -> Vec<(S, R)>
where
    S: Send + Sync + Copy,
    R: Send,
{
    let threads = worker_threads();
    let per_thread: Vec<Vec<(S, R)>> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                s.spawn(move || {
                    let mut out = Vec::new();
                    let mut i = t;
                    while i < specs.len() {
                        out.push((specs[i], play(specs[i])));
                        i += threads;
                    }
                    out
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    per_thread.into_iter().flatten().collect()
}

fn main() {
    // `cargo bench` appends its own `--bench` flag to the binary; drop it so only
    // our own arguments remain.
    let args: Vec<String> = std::env::args().skip(1).filter(|a| a != "--bench").collect();
    match args.first().map(String::as_str) {
        Some("--duel") => {
            if args.len() != 3 {
                eprintln!("usage: --duel <player> <player>");
                std::process::exit(2);
            }
            run_duel(&args[1], &args[2]);
        }
        Some(other) => {
            eprintln!("unknown argument '{other}'. usage: [--duel <player> <player>]");
            std::process::exit(2);
        }
        None => run_league(),
    }
}

// ---------------------------------------------------------------------------
// Round-robin league
// ---------------------------------------------------------------------------

// One scheduled league game: `black`/`white` are indices into LEAGUE_ROSTER.
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

fn play_league(engines: &[(&'static str, Factory)], spec: Spec) -> Outcome {
    // Distinct seed streams so the two engines never share randomness.
    let black = engines[spec.black].1(spec.seed);
    let white = engines[spec.white].1(spec.seed.wrapping_add(1_000_000));
    match GameManager::new(black, white).playout().winner {
        Winner::Black => Outcome::Black,
        Winner::White => Outcome::White,
        Winner::Draw => Outcome::Draw,
    }
}

fn run_league() {
    let engines: Vec<(&'static str, Factory)> = LEAGUE_ROSTER.iter().map(|&n| lookup(n)).collect();
    let n = engines.len();
    let engines_ref = &engines;

    // Build the schedule: every ordered pair, LEAGUE_GAMES seeded games each.
    let mut specs = Vec::new();
    for black in 0..n {
        for white in 0..n {
            if black == white {
                continue;
            }
            for seed in 0..LEAGUE_GAMES {
                specs.push(Spec { black, white, seed });
            }
        }
    }

    let start = Instant::now();
    let results = run_parallel(&specs, move |spec| play_league(engines_ref, spec));
    let elapsed = start.elapsed();

    // head[i][j] = (wins, losses, draws) of engine i vs engine j (both colours).
    let mut head = vec![vec![(0u32, 0u32, 0u32); n]; n];
    for (spec, outcome) in results {
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
    println!("league: {n} engines, {LEAGUE_GAMES} games / ordered pair, {total_games} games total");
    println!("elapsed: {:.2}s ({} threads)", elapsed.as_secs_f64(), worker_threads());
    println!();

    // Head-to-head matrix (row engine's W-L-D vs each column engine).
    print!("{:<10}", "");
    for (name, _) in engines.iter() {
        print!("{name:>10}");
    }
    println!();
    for i in 0..n {
        print!("{:<10}", engines[i].0);
        for j in 0..n {
            if i == j {
                print!("{:>10}", "-");
            } else {
                let (w, l, d) = head[i][j];
                print!("{:>10}", format!("{w}-{l}-{d}"));
            }
        }
        println!();
    }
    println!();

    // Final standings.
    println!("{:<10} {:>4} {:>4} {:>4} {:>7}", "engine", "W", "L", "D", "pts");
    for (i, w, l, d, pts) in standings.iter() {
        println!("{:<10} {:>4} {:>4} {:>4} {:>7.1}", engines[*i].0, w, l, d, pts);
    }
}

// ---------------------------------------------------------------------------
// Head-to-head duel
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct DuelSpec {
    seed: u32,
    a_black: bool,
}

// One game from challenger A's perspective.
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

fn play_duel(a: Factory, b: Factory, spec: DuelSpec) -> GameOut {
    // Distinct seed streams so the two engines never share randomness.
    let mut pa = a(spec.seed);
    let mut pb = b(spec.seed.wrapping_add(1_000_000));
    let mut rng = Xor128::from_seed(spec.seed ^ 0x9E37_79B9);

    // Drive the game manually so the first OPENING_PLIES moves are random
    // (identical for both games of a seed pair), then the players take over.
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
        } else if turn_black == spec.a_black {
            pa.next(&view).expect("legal move exists")
        } else {
            pb.next(&view).expect("legal move exists")
        };

        board = if turn_black { board.flip(mov) } else { board.switch().flip(mov).switch() };
        turn_black = !turn_black;
        ply += 1;
    }

    let (black, white) = board.count();
    let (my, opp) = if spec.a_black { (black, white) } else { (white, black) };
    let margin = my as i32 - opp as i32;
    GameOut { win: margin.signum(), margin }
}

fn run_duel(a_name: &str, b_name: &str) {
    let (a_name, a_fac) = lookup(a_name);
    let (b_name, b_fac) = lookup(b_name);

    // A plays DUEL_GAMES games as black and DUEL_GAMES as white.
    let mut specs = Vec::new();
    for seed in 0..DUEL_GAMES {
        specs.push(DuelSpec { seed, a_black: true });
        specs.push(DuelSpec { seed, a_black: false });
    }

    let start = Instant::now();
    let results = run_parallel(&specs, move |spec| play_duel(a_fac, b_fac, spec));
    let elapsed = start.elapsed();

    // Aggregate overall and split by colour so first-player bias is visible.
    let (mut w, mut l, mut d, mut margin_sum) = (0i32, 0i32, 0i32, 0i64);
    let (mut bw, mut bl, mut bd) = (0i32, 0i32, 0i32); // A as black
    let (mut ww, mut wl, mut wd) = (0i32, 0i32, 0i32); // A as white
    for (spec, g) in results {
        margin_sum += g.margin as i64;
        let (tw, tl, td) =
            if spec.a_black { (&mut bw, &mut bl, &mut bd) } else { (&mut ww, &mut wl, &mut wd) };
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
    println!("duel: {a_name} (challenger) vs {b_name}, {DUEL_GAMES} games / colour, {total} total");
    println!("elapsed: {:.2}s ({} threads)", elapsed.as_secs_f64(), worker_threads());
    println!();
    println!("{a_name} overall:  {w}-{l}-{d}  (W-L-D)   net {:+}", w - l);
    println!("{a_name} as black: {bw}-{bl}-{bd}");
    println!("{a_name} as white: {ww}-{wl}-{wd}");
    println!("{a_name} avg margin: {:+.2} discs", margin_sum as f64 / total as f64);
    println!();
    if w > l {
        println!("=> {a_name} beats {b_name} ({w} > {l}).");
    } else if w < l {
        println!("=> {a_name} LOSES to {b_name} ({w} < {l}).");
    } else {
        println!("=> {a_name} is neutral vs {b_name} ({w} = {l}).");
    }
}
