//! リバーシのプレイヤープログラム (AI) です．
//! 簡単な評価関数を用いた alpha-beta 探索アルゴリズムを実装しています．

pub mod alphabeta;
pub mod cli;
pub mod random;

use reversi::bitboard::*; // fixme: should not depend on bitboard
use reversi::rand;
use reversi::util;
use reversi::{H, W};

use std::cmp::max;
use std::io::{stdout, Write};

/// 十分に大きな値を表す定数です．
const INF: i32 = 100_000_000;

fn position_to_name(r: usize, c: usize) -> String {
    let col_name: Vec<_> = "ABCDEFGH".chars().collect();
    let row_name: Vec<_> = "12345678".chars().collect();
    format!("{}{}", col_name[c], row_name[r])
}

const SEARCH_DEPTH: usize = 7;

pub trait Player {
    fn next(&mut self, board: &Board) -> Option<Mask>;
    fn name(&self) -> &'static str;
}

/// 手番を表します．
#[derive(Clone)]
pub enum Turn {
    Black,
    White,
}

impl Turn {
    /// 自身が黒番なら白番を，白番なら黒番を返します．
    fn switch(&self) -> Turn {
        match self {
            &Turn::Black => Turn::White,
            &Turn::White => Turn::Black,
        }
    }
}

/// ゲームの結果を表す型です．
/// TODO: 手順を追加
#[derive(Clone)]
pub struct GameResult {
    pub winner: Turn,
    pub board: Board,
    pub disks: (u32, u32),
}

pub struct GameManager {
    black: Box<dyn Player>,
    white: Box<dyn Player>,
    board: Board,
    next_player: Turn,
    pub result: Option<GameResult>,
    pub verbose: bool,
}

impl GameManager {
    pub fn new(black: Box<dyn Player>, white: Box<dyn Player>) -> GameManager {
        GameManager {
            black: black,
            white: white,
            board: Board::new(),
            next_player: Turn::Black,
            result: None,
            verbose: true,
        }
    }

    pub fn playout(&mut self) {
        while self.board.continues() {
            if self.verbose {
                println!("==================================================");
            }
            self.next();
        }
        self.finalize();

        if self.verbose {
            let result = self.result.as_ref().expect("game is not finished");
            println!("Final result:");
            print(&result.board);
            let (b, w) = result.disks;
            if b > w {
                println!("First ({}) wins!", self.black.name());
            } else {
                println!("Second ({}) wins!", self.white.name());
            }
            println!(
                "First ({}): {}, Second ({}): {}",
                b,
                self.black.name(),
                w,
                self.white.name()
            );
        }
    }

    fn finalize(&mut self) {
        assert!(self.result.is_none());
        let (black, white) = self.board.count();
        let winner = if black > white {
            Turn::Black
        } else {
            Turn::White
        };
        self.result = Some(GameResult {
            winner: winner,
            board: self.board.clone(),
            disks: (black, white),
        });
    }

    fn next(&mut self) -> Option<Mask> {
        let res = match self.next_player {
            Turn::Black => self.black.next(&self.board),
            Turn::White => self.white.next(&self.board.switch()),
        };
        if let Some(mov) = res {
            debug_assert!(mov.count_ones() == 1);
        };
        self.apply(res);
        res
    }

    fn apply(&mut self, mov: Option<Mask>) {
        match self.next_player {
            Turn::Black => {
                if let Some(mov) = mov {
                    self.board = self.board.reverse(mov);
                    if self.verbose {
                        let (r, c) = movemask_to_position(mov);
                        println!(
                            "First ({}) chooses {}.",
                            self.black.name(),
                            position_to_name(r, c)
                        );
                    }
                } else {
                    if self.verbose {
                        println!("First ({}) passed.", self.black.name());
                    }
                }
            }
            Turn::White => {
                if let Some(mov) = mov {
                    let moved = self.board.switch().reverse(mov).switch();
                    self.board = moved;
                    if self.verbose {
                        let (r, c) = movemask_to_position(mov);
                        println!(
                            "Second ({}) chooses {}.",
                            self.white.name(),
                            position_to_name(r, c)
                        );
                    }
                } else {
                    if self.verbose {
                        println!("Second ({}) passed.", self.white.name());
                    }
                }
            }
        };
        self.next_player = self.next_player.switch();
        if self.verbose {
            let (black, white) = self.board.count();
            println!(
                "{}",
                format!(
                    "{:>16} (First) {:>2} X {:<2} (Second) {:<16}",
                    self.black.name(),
                    black,
                    white,
                    self.white.name()
                )
                .trim()
            );
        }
    }

    // fn get_move(player: &mut Player, board: &Board) -> Option<Mask> {
    //     match *player {
    //         Player::Random(ref mut p) => p.next(&board),
    //         Player::AlphaBeta(ref mut p) => p.next(&board),
    //         Player::Human(ref mut p) => p.next(&board),
    //     }
    // }
}

// todo: move to cli
/// 標準出力に出力します．
fn print(board: &Board) {
    let (valid, _) = board.get_valid_mask();
    let mut g = empty_grid();
    write_mask_to(&mut g, board.0, 'X');
    write_mask_to(&mut g, board.1, 'O');
    write_mask_to(&mut g, valid, '.');
    for row in g.iter() {
        println!("{}", row.iter().collect::<String>());
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
