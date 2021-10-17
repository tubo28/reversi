use reversi::bitboard::*;
use reversi::player::*;
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
        let markers = "123456789qwertyuipasdfghjklzcvbnmQWERTYUIPASDFGHJKLZCVBNM+-*/=()";
        debug_assert_eq!(markers.len(), H * W);
        let mut markers = markers.chars().rev().collect::<Vec<_>>();
        let mut map = BTreeMap::new();
        let mut cand = Vec::new();

        {
            let mut g = empty_grid();
            write_mask_to(&mut g, board.0, 'X');
            write_mask_to(&mut g, board.1, 'O');
            for j in 0..H {
                for i in 0..H {
                    if get(black_moves, i, j) {
                        let c = markers.pop().unwrap();
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

// todo: remove dups
fn write_mask_to(g: &mut Vec<Vec<char>>, mask: Mask, c: char) {
    for i in 0..H * 2 + 1 {
        for j in 0..W * 2 + 1 {
            if i % 2 == 1 && j % 2 == 1 {
                if get(mask, i / 2, j / 2) {
                    debug_assert_eq!(g[i][j], ' ');
                    g[i][j] = c;
                }
            }
        }
    }
}

// todo: remove dups
fn empty_grid() -> Vec<Vec<char>> {
    let mut g = vec![vec![' '; H * 2 + 1]; W * 2 + 1];
    for i in 0..H * 2 + 1 {
        for j in 0..W * 2 + 1 {
            if i % 2 != 1 || j % 2 != 1 {
                debug_assert_eq!(g[i][j], ' ');
                g[i][j] = if i % 2 == 0 && j % 2 == 0 {
                    '+'
                } else if i % 2 == 0 {
                    '-'
                } else {
                    '|'
                };
            }
        }
    }
    g
}
