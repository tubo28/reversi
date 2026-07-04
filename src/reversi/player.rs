pub mod alphabeta;
pub mod alphabeta2;
pub mod alphabeta3;
pub mod alphabeta4;
pub mod alphabeta42;
pub mod alphabeta5;
pub mod best;
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
