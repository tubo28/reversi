//! Generator for "guaranteed-win" endgame positions, used by the sprint mode in
//! the web UI (see `web/src/index.ts`).
//!
//! `AlphaBeta5` plays itself from the opening down to a target number of empty
//! cells, then the exact endgame solver ([`AlphaBeta5Player::solve_exact`]) is
//! run from the resulting position. If the side to move has a *proven* forced win
//! (exact final disk difference > 0), that position is returned; the human then
//! plays it out from their (winning) turn against the engine.
//!
//! The board is kept in the engine's usual "side to move is `board.0`"
//! perspective throughout, so one ply is `board.flip(mov).switch()` and the
//! returned position's `.0` is exactly the side that is to move (the future
//! human). This mirrors the manual driving pattern in `benches/duel5.rs`.

use crate::reversi::bitboard::{legal_moves, Board};
use crate::reversi::player::alphabeta5::AlphaBeta5Player;
use crate::reversi::player::Player;

/// A position where the side to move (`me`) has a proven forced win by `margin`
/// disks under perfect play. `me`/`opp` are raw bitmasks from the mover's
/// perspective (`me` = side to move).
pub struct WinPosition {
    pub me: u64,
    pub opp: u64,
    pub margin: i32,
}

/// Generates a position where the side to move has a forced win, confirmed by an
/// exact endgame solve. Returns `None` only if no such position was found within
/// `max_attempts` self-play games.
///
/// - `target_empties`: empty-cell count at which self-play stops and the puzzle
///   begins (fewer = shorter/easier and faster to solve).
/// - `gen_budget`: per-move node budget for the self-play `AlphaBeta5` (kept low
///   so a full self-play game is fast; strength is irrelevant to correctness).
/// - `solve_budget`: node budget for the verification solve. Must be large enough
///   that the exact tree is exhausted at `target_empties`, otherwise the solve
///   aborts and the (untrusted) attempt is skipped.
/// - `max_attempts`: how many self-play games (each with a fresh seed) to try.
pub fn generate_win_position(
    seed: u32,
    target_empties: u32,
    gen_budget: u64,
    solve_budget: u64,
    max_attempts: u32,
) -> Option<WinPosition> {
    for attempt in 0..max_attempts {
        // Spread the per-attempt seed so distinct self-play lines are explored
        // (the deep search has few ties, so the seed is what decorrelates games).
        let s = seed.wrapping_add(attempt).wrapping_mul(0x9E37_79B1);

        let view = match play_to_empties(s, target_empties, gen_budget) {
            Some(v) => v,
            None => continue, // game ended before reaching the target
        };

        // The human must actually have a move at the start of the puzzle.
        if legal_moves(view.0, view.1) == 0 {
            continue;
        }

        let mut solver = AlphaBeta5Player::with_budget(s, solve_budget);
        // A `None` here means the solve was aborted (budget exhausted): never
        // trust it as a verdict. Only a completed solve with a positive margin is
        // a proven forced win for the side to move.
        if let Some(margin) = solver.solve_exact(&view) {
            if margin > 0 {
                return Some(WinPosition { me: view.0, opp: view.1, margin });
            }
        }
    }
    None
}

/// Plays `AlphaBeta5` against itself from the opening until exactly
/// `target_empties` empty cells remain, returning the board in mover-perspective
/// (`.0` = side to move). Returns `None` if the game ends (both sides pass)
/// before the target is reached.
fn play_to_empties(seed: u32, target_empties: u32, gen_budget: u64) -> Option<Board> {
    // One player instance drives both sides: `next()` clears its search table and
    // decays history every call and does not touch the solve table this far from
    // the end, so sharing it is exactly its normal move-to-move behaviour.
    let mut ai = AlphaBeta5Player::with_budget(seed, gen_budget);
    let mut board = Board::new();
    loop {
        // Each move fills exactly one empty cell, so `empties` hits the target
        // exactly (a pass does not change the board).
        let empties = 64 - (board.0 | board.1).count_ones();
        if empties == target_empties {
            return Some(board);
        }
        if legal_moves(board.0, board.1) == 0 {
            board = board.switch(); // current side must pass
            if legal_moves(board.0, board.1) == 0 {
                return None; // both sides pass: game over before the target
            }
            continue;
        }
        match ai.next(&board) {
            Some(mov) => board = board.flip(mov).switch(), // play; opponent becomes .0
            None => board = board.switch(),                // defensive; shouldn't happen
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Calibration / correctness gate: at a modest depth the generator should find
    // a confirmed forced-win position, and the returned position must genuinely be
    // a completed exact win for the side to move.
    #[test]
    fn generates_a_confirmed_win() {
        const TARGET: u32 = 12;
        const GEN_BUDGET: u64 = 120_000;
        const SOLVE_BUDGET: u64 = 40_000_000;

        let win = generate_win_position(1, TARGET, GEN_BUDGET, SOLVE_BUDGET, 20)
            .expect("should find a forced-win endgame within 20 attempts");

        // The position is the human's turn and they actually have a move.
        assert_ne!(legal_moves(win.me, win.opp), 0);
        // The reported margin must be an independently reproducible, completed win.
        let mut solver = AlphaBeta5Player::with_budget(1, SOLVE_BUDGET);
        let exact =
            solver.solve_exact(&Board(win.me, win.opp)).expect("verification solve must complete");
        assert!(exact > 0, "returned position must be a forced win, got {exact}");
        assert_eq!(exact, win.margin);
        // And it is at the requested depth.
        assert_eq!(64 - (win.me | win.opp).count_ones(), TARGET);
    }

    // Calibration (run explicitly): per-game to-move forced-win rate and solve
    // completion rate at each difficulty, to justify MAX_ATTEMPTS and estimate
    // generation time. Run with:
    //   cargo test --release --lib sprint::tests::calibrate -- --ignored --nocapture
    #[test]
    #[ignore]
    fn calibrate() {
        const GEN_BUDGET: u64 = 120_000;
        const SOLVE_BUDGET: u64 = 40_000_000;
        const SAMPLES: u32 = 80;
        for &target in &[12u32, 14, 16] {
            let start = std::time::Instant::now();
            let (mut wins, mut solved, mut ended) = (0u32, 0u32, 0u32);
            for seed in 0..SAMPLES {
                let s = seed.wrapping_mul(0x9E37_79B1).wrapping_add(1);
                let view = match play_to_empties(s, target, GEN_BUDGET) {
                    Some(v) => v,
                    None => {
                        ended += 1;
                        continue;
                    }
                };
                if legal_moves(view.0, view.1) == 0 {
                    continue;
                }
                let mut solver = AlphaBeta5Player::with_budget(s, SOLVE_BUDGET);
                if let Some(margin) = solver.solve_exact(&view) {
                    solved += 1;
                    if margin > 0 {
                        wins += 1;
                    }
                }
            }
            let dt = start.elapsed();
            eprintln!(
                "empties={target}: to-move win {wins}/{SAMPLES} ({:.0}%), solve completed {solved}/{SAMPLES}, early-end {ended}, {:.0}ms/game (native)",
                100.0 * wins as f64 / SAMPLES as f64,
                dt.as_millis() as f64 / SAMPLES as f64,
            );
        }
    }
}
