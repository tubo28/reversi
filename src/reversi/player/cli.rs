use crate::reversi::asciiboard;
use crate::reversi::bitboard::*;
use crate::reversi::player::*;
use std::collections::BTreeMap;
use std::io::*;

/// Player by human's input.
/// Helps inputs in CLI.
pub struct HumanPlayer;

impl HumanPlayer {
    #[allow(dead_code)]
    pub fn new() -> HumanPlayer {
        HumanPlayer
    }
}

impl Player for HumanPlayer {
    fn next(&mut self, board: &Board) -> Option<Mask> {
        let (black_moves, _) = board.get_valid_mask();
        let markers = "123456789qwertyuipasdfghjklzcvbnmQWERTYUIPASDFGHJKLZCVBNM+-*/=()"
            .chars()
            .rev()
            .collect::<Vec<_>>();
        debug_assert_eq!(markers.len(), H * W);
        let mut map = BTreeMap::new();
        let mut cand = Vec::new();
        let mut num_used_markers = 0;

        {
            let mut g = asciiboard::empty();
            asciiboard::write_mask(&mut g, board.0, asciiboard::BLACK_MARK);
            asciiboard::write_mask(&mut g, board.1, asciiboard::WHITE_MARK);
            for j in 0..H {
                for i in 0..H {
                    if get(black_moves, i, j) {
                        let c = markers[num_used_markers];
                        num_used_markers += 1;
                        g[i * 2 + 1][j * 2 + 1] = c;
                        map.insert(c, (i, j));
                        cand.push(c);
                    }
                }
            }
            for row in g.iter() {
                println!("{}", row.iter().collect::<String>());
            }
        }

        if black_moves == 0 {
            None
        } else {
            let mut c = None;
            while c.is_none() || map.get(c.as_ref().unwrap()).is_none() {
                println!("Possible moves are:");
                for (k, &(r, c)) in map.iter() {
                    println!("  {} : {}", k, util::position_to_name(r, c));
                }
                print!("Type any character of [{}]: ", cand.iter().collect::<String>());
                stdout().flush().unwrap();
                c = util::read_one_char();
            }
            let (r, c) = map[&c.unwrap()];
            Some(position_to_mask(r, c))
        }
    }

    fn name(&self) -> &'static str {
        "Human"
    }
}
