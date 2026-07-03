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
use crate::reversi::player::alphabeta42::AlphaBeta42Player;
use crate::reversi::player::Player;

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
/// move (the player must pass). `seed` seeds the AI's move randomization.
#[no_mangle]
pub extern "C" fn ai_move(black: u64, white: u64, seed: u32) -> u64 {
    AlphaBeta42Player::new(seed)
        .next(&Board(black, white))
        .unwrap_or(0)
}
