use crate::reversi::asciiboard;
use crate::reversi::bitboard;
use crate::reversi::player::Player;
use crate::reversi::util;

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
            asciiboard::print(&result.board);
            let (b, w) = result.disks;
            if b > w {
                println!("first ({}) wins!", self.black.name());
            } else {
                println!("second ({}) wins!", self.white.name());
            }
            println!("first ({}): {}, second ({}): {}", b, self.black.name(), w, self.white.name());
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
                            "first ({}) chooses {}.",
                            self.black.name(),
                            util::position_to_name(r, c)
                        );
                    }
                } else {
                    if self.verbose {
                        println!("first ({}) passed.", self.black.name());
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
                            "second ({}) chooses {}.",
                            self.white.name(),
                            util::position_to_name(r, c)
                        );
                    }
                } else {
                    if self.verbose {
                        println!("second ({}) passed.", self.white.name());
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
                    "{:>16} {:>2} X {:<2} {:<16}",
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
