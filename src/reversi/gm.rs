use reversi::bitboard;
use reversi::player::Player;
use reversi::util;
use reversi::{H, W};

/// Player who will take the next move.
#[derive(Clone)]
pub enum Turn {
    Black,
    White,
}

impl Turn {
    fn switch(&self) -> Turn {
        match self {
            &Turn::Black => Turn::White,
            &Turn::White => Turn::Black,
        }
    }
}

/// The result of a game
/// TODO: Add sequence of moves.
#[derive(Clone)]
pub struct GameResult {
    pub winner: Turn,

    // Which player will move it next
    pub board: bitboard::Board,

    // Numbers of disks
    pub disks: (u32, u32),
}

pub struct GameManager {
    black: Box<dyn Player>,
    white: Box<dyn Player>,

    // Current state of board
    board: bitboard::Board,

    // Which player can put a disk in current state
    next_player: Turn,

    // Result of the game.
    // None if the game is not finished, Some for otherwise.
    pub result: Option<GameResult>,

    // Print verbose output of game state while the game goes on.
    pub verbose: bool,
}

impl GameManager {
    pub fn new(black: Box<dyn Player>, white: Box<dyn Player>) -> GameManager {
        GameManager {
            black,
            white,
            // Black is first to move.
            board: bitboard::Board::new(),
            next_player: Turn::Black,
            result: None,
            verbose: true,
        }
    }

    // Start the game and continue until it is over.
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
            println!("First ({}): {}, Second ({}): {}", b, self.black.name(), w, self.white.name());
        }
    }

    // Fill result field.
    fn finalize(&mut self) {
        assert!(self.result.is_none());
        let (black, white) = self.board.count();
        let winner = if black > white { Turn::Black } else { Turn::White };
        self.result =
            Some(GameResult { winner: winner, board: self.board.clone(), disks: (black, white) });
    }

    // Let next player choose the move.
    // Returns None only if there is no choice but to pass.
    fn next(&mut self) -> Option<bitboard::Mask> {
        let res = match self.next_player {
            Turn::Black => self.black.next(&self.board),
            Turn::White => self.white.next(&self.board.switch()),
        };
        // TODO: If None, we should check that is ok.
        if let Some(mov) = res {
            debug_assert!(mov.count_ones() == 1);
        };
        self.apply(res);
        res
    }

    // Apply a move and update self.
    fn apply(&mut self, mov: Option<bitboard::Mask>) {
        match self.next_player {
            Turn::Black => {
                if let Some(mov) = mov {
                    self.board = self.board.flip(mov);
                    if self.verbose {
                        let (r, c) = bitboard::coordinate(mov);
                        println!(
                            "First ({}) chooses {}.",
                            self.black.name(),
                            util::position_to_name(r, c)
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
                    let moved = self.board.switch().flip(mov).switch();
                    self.board = moved;
                    if self.verbose {
                        let (r, c) = bitboard::coordinate(mov);
                        println!(
                            "Second ({}) chooses {}.",
                            self.white.name(),
                            util::position_to_name(r, c)
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
}

// todo: move to cli
/// 標準出力に出力します．
fn print(board: &bitboard::Board) {
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
fn write_mask_to(g: &mut Vec<Vec<char>>, mask: bitboard::Mask, c: char) {
    for i in 0..H * 2 + 1 {
        for j in 0..W * 2 + 1 {
            if i % 2 == 1 && j % 2 == 1 {
                if bitboard::get(mask, i / 2, j / 2) {
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
