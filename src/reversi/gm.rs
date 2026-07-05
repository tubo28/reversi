use crate::reversi::asciiboard;
use crate::reversi::bitboard::{coordinate, Board, Mask};
use crate::reversi::player::Player;
use crate::reversi::util;

/// Player who will take the next move.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Turn {
    Black,
    White,
}

impl Turn {
    /// The other player's turn.
    fn switch(self) -> Turn {
        match self {
            Turn::Black => Turn::White,
            Turn::White => Turn::Black,
        }
    }

    /// The ascii mark of the disks this player places.
    fn mark(self) -> char {
        match self {
            Turn::Black => asciiboard::BLACK_MARK,
            Turn::White => asciiboard::WHITE_MARK,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Winner {
    Black,
    White,
    Draw,
}

/// The result of a game.
/// TODO: Add sequence of moves.
#[derive(Clone)]
pub struct GameResult {
    pub winner: Winner,

    // Final state of the board.
    pub board: Board,

    // Numbers of disks (black, white).
    pub disks: (u32, u32),
}

// ---------------------------------------------------------------------------
// Pure game logic (no state, no side effects).
// ---------------------------------------------------------------------------

/// The board as seen by the player to move: routines always assume black is to
/// move, so white plays on the colour-swapped board.
fn perspective(board: &Board, turn: Turn) -> Board {
    match turn {
        Turn::Black => board.clone(),
        Turn::White => board.switch(),
    }
}

/// Applies `turn`'s move to `board`, returning the new board. `mov` is `None`
/// for a pass (board unchanged) or a single-bit mask in the mover's own
/// perspective.
fn play_move(board: &Board, turn: Turn, mov: Option<Mask>) -> Board {
    match (turn, mov) {
        (_, None) => board.clone(),
        (Turn::Black, Some(mov)) => board.flip(mov),
        (Turn::White, Some(mov)) => board.switch().flip(mov).switch(),
    }
}

/// Decides the winner from the disk counts of a (usually finished) board.
fn winner_of(board: &Board) -> Winner {
    let (black, white) = board.count();
    match black.cmp(&white) {
        std::cmp::Ordering::Greater => Winner::Black,
        std::cmp::Ordering::Equal => Winner::Draw,
        std::cmp::Ordering::Less => Winner::White,
    }
}

/// Builds the result of a finished game from its final board.
fn finalize(board: &Board) -> GameResult {
    GameResult { winner: winner_of(board), board: board.clone(), disks: board.count() }
}

// ---------------------------------------------------------------------------
// Game driver (the only stateful part is the players themselves).
// ---------------------------------------------------------------------------

pub struct GameManager {
    black: Box<dyn Player>,
    white: Box<dyn Player>,
}

impl GameManager {
    pub fn new(black: Box<dyn Player>, white: Box<dyn Player>) -> GameManager {
        GameManager { black, white }
    }

    /// Plays the game to the end silently and returns the result.
    pub fn playout(&mut self) -> GameResult {
        self.run(None)
    }

    /// Plays the game to the end, printing each move and the final result, then
    /// returns the result.
    pub fn playout_verbose(&mut self) -> GameResult {
        let reporter = Reporter { black: self.black.name(), white: self.white.name() };
        self.run(Some(&reporter))
    }

    // Folds board transitions from the opening until neither side can move.
    // `reporter`, when present, is the sole sink for stdout side effects.
    fn run(&mut self, reporter: Option<&Reporter>) -> GameResult {
        let mut board = Board::new();
        let mut turn = Turn::Black;

        while board.continues() {
            if let Some(r) = reporter {
                r.separator();
            }

            let view = perspective(&board, turn);
            let mov = match turn {
                Turn::Black => self.black.next(&view),
                Turn::White => self.white.next(&view),
            };
            if let Some(mov) = mov {
                debug_assert!(mov.count_ones() == 1);
            }

            board = play_move(&board, turn, mov);
            if let Some(r) = reporter {
                r.ply(turn, mov, &board);
            }
            turn = turn.switch();
        }

        let result = finalize(&board);
        if let Some(r) = reporter {
            r.result(&result);
        }
        result
    }
}

// ---------------------------------------------------------------------------
// Side effects (stdout) are isolated here.
// ---------------------------------------------------------------------------

struct Reporter {
    black: &'static str,
    white: &'static str,
}

impl Reporter {
    fn name(&self, turn: Turn) -> &'static str {
        match turn {
            Turn::Black => self.black,
            Turn::White => self.white,
        }
    }

    fn separator(&self) {
        println!("==================================================");
    }

    // Reports the move (or pass) `turn` just made, followed by the score line.
    fn ply(&self, turn: Turn, mov: Option<Mask>, board: &Board) {
        let colour = match turn {
            Turn::Black => "black",
            Turn::White => "white",
        };
        match mov {
            Some(mov) => {
                let (r, c) = coordinate(mov);
                println!(
                    "{} ({}) chooses {}.",
                    colour,
                    self.name(turn),
                    util::position_to_name(r, c)
                );
            }
            None => println!("{} ({}) passed.", colour, self.name(turn)),
        }

        let (black, white) = board.count();
        println!(
            "{}",
            format!("{:>16} {:>2} X {:<2} {:<16}", self.black, black, white, self.white).trim()
        );
    }

    fn result(&self, result: &GameResult) {
        println!("Final result:");
        asciiboard::print(&result.board);
        let (black, white) = result.disks;
        println!("{} black ({}): {}", Turn::Black.mark(), self.black, black);
        println!("{} white ({}): {}", Turn::White.mark(), self.white, white);
        println!("winner: {:?}", result.winner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reversi::player::random::RandomPlayer;

    #[test]
    fn turn_switch_toggles() {
        assert_eq!(Turn::Black.switch(), Turn::White);
        assert_eq!(Turn::White.switch(), Turn::Black);
    }

    #[test]
    fn play_move_black_opening() {
        // From the opening, black plays cell (2, 3) = bit 19, one of the four
        // legal openings, flipping a single white disc.
        let board = play_move(&Board::new(), Turn::Black, Some(1 << 19));
        assert_eq!(board.count(), (4, 1));
    }

    #[test]
    fn play_move_pass_is_identity() {
        let board = Board::new();
        let passed = play_move(&board, Turn::Black, None);
        assert_eq!((passed.0, passed.1), (board.0, board.1));
    }

    #[test]
    fn play_move_white_perspective() {
        // Black opens, then white replies from its swapped perspective; the move
        // count must grow by one each ply (opening flips only a single disc).
        let after_black = play_move(&Board::new(), Turn::Black, Some(1 << 19));
        assert_eq!(after_black.count(), (4, 1));

        // White to move: legal moves are computed on the swapped board.
        let white_view = perspective(&after_black, Turn::White);
        let (white_moves, _) = white_view.get_valid_mask();
        assert_ne!(white_moves, 0);
        let mov = white_moves & white_moves.wrapping_neg();

        let after_white = play_move(&after_black, Turn::White, Some(mov));
        let (b, w) = after_white.count();
        assert_eq!(b + w, 6, "one white placement plus its flips add exactly one net disc");
    }

    #[test]
    fn winner_of_reads_majority() {
        // Two black discs, one white disc.
        assert_eq!(winner_of(&Board(0b11, 0b100)), Winner::Black);
        assert_eq!(winner_of(&Board(0b100, 0b11)), Winner::White);
        assert_eq!(winner_of(&Board(0b1, 0b10)), Winner::Draw);
    }

    #[test]
    fn finalize_reports_counts_and_winner() {
        let result = finalize(&Board(0b111, 0b1000));
        assert_eq!(result.disks, (3, 1));
        assert_eq!(result.winner, Winner::Black);
    }

    #[test]
    fn playout_random_is_deterministic_and_terminal() {
        let play = || {
            let mut gm =
                GameManager::new(Box::new(RandomPlayer::new(42)), Box::new(RandomPlayer::new(7)));
            gm.playout()
        };

        let first = play();
        let second = play();

        // Same seeds reproduce the same outcome.
        assert_eq!(first.winner, second.winner);
        assert_eq!(first.disks, second.disks);

        // Result is consistent and the board is genuinely finished.
        let (black, white) = first.disks;
        assert_eq!(first.board.count(), first.disks);
        assert!(black + white <= 64);
        assert!(!first.board.continues(), "playout must stop at a terminal board");
    }
}
