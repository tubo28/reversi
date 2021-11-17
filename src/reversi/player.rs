pub mod alphabeta;
pub mod cli;
pub mod random;

use crate::reversi::bitboard;
use crate::reversi::rand;
use crate::reversi::util;
use crate::reversi::{H, W};
use std::cmp::max;

/// Trait for reversi player.
/// It can decide the next move and say their name.
pub trait Player {
    // Select a move by given board.
    // None is pass (allowed only if there is no valid moves).
    fn next(&mut self, board: &bitboard::Board) -> Option<bitboard::Mask>;
    fn name(&self) -> &'static str;
}
