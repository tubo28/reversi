use crate::reversi::bitboard::*;
use crate::reversi::player::*;
use crate::reversi::{H, W};

/// Player who always does random moves.
pub struct RandomPlayer {
    rand: rand::Xor128,
}

impl RandomPlayer {
    pub fn new(seed: u32) -> RandomPlayer {
        RandomPlayer { rand: rand::Xor128::from_seed(seed) }
    }
}

impl Player for RandomPlayer {
    fn next(&mut self, board: &Board) -> Option<Mask> {
        let (black_moves, _) = board.get_valid_mask();
        // board.print();
        // println!("{:064b}", black_moves);
        if black_moves == 0 {
            None
        } else {
            let mut best = (u32::min_value(), 0);
            for mov in (0..H * W).map(|i| 1 << i).filter(|&m| black_moves & m == m) {
                best = max(best, (self.rand.next() + 1, mov));
            }
            let (_, best_position) = best;
            debug_assert!(best_position != 0);
            Some(best_position)
        }
    }

    fn name(&self) -> &'static str {
        "Random"
    }
}
