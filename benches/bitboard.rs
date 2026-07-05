//! Micro-benchmark for the bitboard core: legal-move generation (`legal_moves`)
//! and disc flipping (`flip_discs`). Collects a large set of reachable positions
//! by random self-play, checks the routines against an obvious scalar oracle,
//! then reports throughput (ns/op).
//!
//! Run with: `cargo bench --bench bitboard`  (declared with `harness = false`).

use std::hint::black_box;
use std::time::{Duration, Instant};

use reversi::reversi::bitboard::{flip_discs, legal_moves, Board, Mask};
use reversi::reversi::rand::Xor128;

/// A position plus its legal moves, collected from random self-play.
struct Case {
    board: Board,
    moves: Mask,
}

const DIRS: [(i32, i32); 8] =
    [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)];

#[inline]
fn bit(mask: Mask, r: i32, c: i32) -> bool {
    mask >> (r * 8 + c) & 1 == 1
}

/// Obvious-by-inspection scalar oracle for the legal moves of black.
fn legal_moves_ref(black: Mask, white: Mask) -> Mask {
    let occupied = black | white;
    let mut moves = 0;
    for r in 0..8 {
        for c in 0..8 {
            if occupied >> (r * 8 + c) & 1 == 1 {
                continue;
            }
            for &(dr, dc) in DIRS.iter() {
                let (mut nr, mut nc) = (r + dr, c + dc);
                let mut seen_opp = false;
                loop {
                    if !(0..8).contains(&nr) || !(0..8).contains(&nc) {
                        break;
                    }
                    if bit(white, nr, nc) {
                        seen_opp = true;
                        nr += dr;
                        nc += dc;
                    } else if bit(black, nr, nc) {
                        if seen_opp {
                            moves |= 1u64 << (r * 8 + c);
                        }
                        break;
                    } else {
                        break;
                    }
                }
            }
        }
    }
    moves
}

/// Obvious-by-inspection scalar oracle for the discs flipped by black at `mov`.
fn flip_ref(black: Mask, white: Mask, mov: Mask) -> Mask {
    let idx = mov.trailing_zeros() as i32;
    let (r, c) = (idx / 8, idx % 8);
    let mut flipped = 0;
    for &(dr, dc) in DIRS.iter() {
        let (mut nr, mut nc) = (r + dr, c + dc);
        let mut run = 0u64;
        loop {
            if !(0..8).contains(&nr) || !(0..8).contains(&nc) {
                run = 0;
                break;
            }
            if bit(white, nr, nc) {
                run |= 1u64 << (nr * 8 + nc);
                nr += dr;
                nc += dc;
            } else if bit(black, nr, nc) {
                break;
            } else {
                run = 0;
                break;
            }
        }
        flipped |= run;
    }
    flipped
}

/// Collect a large, varied set of reachable positions via random playouts.
fn collect_positions(target: usize) -> Vec<Case> {
    let mut rng = Xor128::from_seed(98765);
    let mut cases = Vec::with_capacity(target);
    while cases.len() < target {
        let mut board = Board::new();
        let mut passed = false;
        loop {
            let moves = legal_moves(board.0, board.1);
            if moves == 0 {
                if passed {
                    break;
                }
                board = board.switch();
                passed = true;
                continue;
            }
            passed = false;
            cases.push(Case { board: board.clone(), moves });
            if cases.len() >= target {
                break;
            }
            let mut choices: Vec<Mask> = Vec::new();
            let mut m = moves;
            while m != 0 {
                choices.push(m & m.wrapping_neg());
                m &= m - 1;
            }
            let mov = choices[rng.next() as usize % choices.len()];
            board = board.flip(mov).switch();
        }
    }
    cases
}

fn main() {
    let cases = collect_positions(100_000);
    println!("bitboard micro-bench: {} positions", cases.len());

    // --- Correctness against the scalar oracle. ---
    for c in cases.iter() {
        assert_eq!(legal_moves(c.board.0, c.board.1), legal_moves_ref(c.board.0, c.board.1));
        let mut m = c.moves;
        while m != 0 {
            let mov = m & m.wrapping_neg();
            assert_eq!(flip_discs(c.board.0, c.board.1, mov), flip_ref(c.board.0, c.board.1, mov));
            m &= m - 1;
        }
    }
    println!("correctness: matches scalar oracle on all positions and moves\n");

    const ITERS: u32 = 50;
    let ops = cases.len() as u64 * ITERS as u64;
    let ns = |d: Duration| d.as_nanos() as f64 / ops as f64;

    let start = Instant::now();
    let mut acc = 0u64;
    for _ in 0..ITERS {
        for c in cases.iter() {
            acc ^= legal_moves(black_box(c.board.0), black_box(c.board.1));
        }
    }
    black_box(acc);
    let mob = start.elapsed();

    let start = Instant::now();
    let mut acc = 0u64;
    for _ in 0..ITERS {
        for c in cases.iter() {
            let mov = c.moves & c.moves.wrapping_neg();
            acc ^= flip_discs(black_box(c.board.0), black_box(c.board.1), black_box(mov));
        }
    }
    black_box(acc);
    let flip = start.elapsed();

    println!("legal-move generation : {:6.2} ns/op", ns(mob));
    println!("disc flip             : {:6.2} ns/op", ns(flip));
}
