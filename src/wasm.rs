//! WebAssembly API for the reversi engine.
//!
//! All functions work from the "black to move" perspective, matching the
//! engine's internal convention (see `Board::switch`). When it is white's turn,
//! the JS caller swaps the arguments (`valid_moves(white, black)`); move masks
//! are positions and do not depend on color, so swapping is correct.
//!
//! Every function returns a single `u64` so nothing needs to be passed back
//! through pointers into linear memory. The full board is two `u64`s, so the
//! JS side keeps `(black, white)` as BigInts and reconstructs the next board
//! from the flip mask returned by [`flip_mask`].
use crate::reversi::bitboard::Board;
use crate::reversi::player::alphabeta5::AlphaBeta5Player;
use crate::reversi::player::Player;
use crate::reversi::sprint::generate_win_position;
use std::cell::RefCell;

// Sprint-generation tuning (see `src/reversi/sprint.rs`). The verdict is proven
// by exact tree exhaustion, so these only trade generation time against how often
// a game yields a to-move forced win — never the correctness of the win.
const GEN_BUDGET: u64 = 120_000; // per-move node budget for the self-play AB5
const SOLVE_BUDGET: u64 = 40_000_000; // node budget for the verification solve
const MAX_ATTEMPTS: u32 = 12; // self-play games tried before giving up

thread_local! {
    // A single persistent AI so its (safe-to-carry) endgame solve table survives
    // across moves within a game. wasm32 is single-threaded, so this thread-local
    // is effectively a global. Re-created whenever the caller changes `seed`.
    static AI: RefCell<Option<(u32, AlphaBeta5Player)>> = const { RefCell::new(None) };

    // Stash for the most recent `generate_endgame` result: (me, opp, margin) from
    // the mover's perspective (`me` = side to move = human). Read back through the
    // getters below, since each extern fn can only return a single u64.
    static GENERATED: RefCell<Option<(u64, u64, i32)>> = const { RefCell::new(None) };
}

/// Mask of cells where the black (to-move) player may put a disk.
#[no_mangle]
pub extern "C" fn valid_moves(black: u64, white: u64) -> u64 {
    Board(black, white).get_valid_mask().0
}

/// Mask of white disks that get flipped when black plays `mov`.
///
/// `mov` must be a single-bit mask of a legal move. Returns 0 for a pass
/// (`mov == 0`) or an illegal move, so the caller can validate cheaply.
///
/// The JS side reconstructs the next board as:
///   new_black = black | mov | flip
///   new_white = white ^ flip
#[no_mangle]
pub extern "C" fn flip_mask(black: u64, white: u64, mov: u64) -> u64 {
    let board = Board(black, white);
    let (valid, hints) = board.get_valid_mask();
    if mov == 0 || mov & valid != mov {
        return 0;
    }
    let after = board.flip_with_hints(mov, &hints);
    white ^ after.1
}

/// Best move mask for the black (to-move) player, or 0 if there is no legal
/// move (the player must pass). `seed` seeds the AI's move randomization; the
/// persistent player is rebuilt whenever `seed` changes (i.e. a new game).
#[no_mangle]
pub extern "C" fn ai_move(black: u64, white: u64, seed: u32) -> u64 {
    AI.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.as_ref().map(|(s, _)| *s) != Some(seed) {
            *slot = Some((seed, AlphaBeta5Player::new(seed)));
        }
        let (_, ai) = slot.as_mut().unwrap();
        ai.next(&Board(black, white)).unwrap_or(0)
    })
}

/// Generates a "sprint" endgame position with `target_empties` empty cells in
/// which the side to move has a *proven* forced win (confirmed by exact endgame
/// search), via AlphaBeta5-vs-AlphaBeta5 self-play. Returns 1 on success (the
/// position is stashed; read it with [`generated_black`] / [`generated_white`] /
/// [`generated_margin`]) or 0 if none was found. The stashed board is from the
/// mover's perspective, so the human should be set up as the side to move.
#[no_mangle]
pub extern "C" fn generate_endgame(seed: u32, target_empties: u32) -> u64 {
    let result = generate_win_position(seed, target_empties, GEN_BUDGET, SOLVE_BUDGET, MAX_ATTEMPTS);
    GENERATED.with(|cell| {
        *cell.borrow_mut() = result.as_ref().map(|w| (w.me, w.opp, w.margin));
    });
    if result.is_some() {
        1
    } else {
        0
    }
}

/// The side-to-move (human) disks of the last successful [`generate_endgame`].
#[no_mangle]
pub extern "C" fn generated_black() -> u64 {
    GENERATED.with(|cell| cell.borrow().map(|(me, _, _)| me).unwrap_or(0))
}

/// The opponent (AI) disks of the last successful [`generate_endgame`].
#[no_mangle]
pub extern "C" fn generated_white() -> u64 {
    GENERATED.with(|cell| cell.borrow().map(|(_, opp, _)| opp).unwrap_or(0))
}

/// The proven forced-win margin (final disc difference) of the last successful
/// [`generate_endgame`]; positive on success. Returned as `u64` per the C ABI.
#[no_mangle]
pub extern "C" fn generated_margin() -> u64 {
    GENERATED.with(|cell| cell.borrow().map(|(_, _, m)| m as i64 as u64).unwrap_or(0))
}
