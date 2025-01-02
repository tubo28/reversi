use crate::reversi::bitboard;
use crate::reversi::{H, W};

pub const BLACK_MARK: char = 'x';
pub const WHITE_MARK: char = 'o';
const VALID_MOVE_MARK: char = '.';
const EMPTY_MARK: char = ' ';

/// Returns new empty board.
pub fn empty() -> Vec<Vec<char>> {
    let mut grid = vec![vec![EMPTY_MARK; H * 2 + 1]; W * 2 + 1];
    for i in 0..H * 2 + 1 {
        for j in 0..W * 2 + 1 {
            if i % 2 != 1 || j % 2 != 1 {
                debug_assert_eq!(grid[i][j], EMPTY_MARK);
                grid[i][j] = if i % 2 == 0 && j % 2 == 0 {
                    '+'
                } else if i % 2 == 0 {
                    '-'
                } else {
                    '|'
                };
            }
        }
    }
    grid
}

/// Write bitboard mask to grid by specific char.
pub fn write_mask(grid: &mut Vec<Vec<char>>, mask: bitboard::Mask, c: char) {
    for i in 0..H * 2 + 1 {
        for j in 0..W * 2 + 1 {
            if i % 2 == 1 && j % 2 == 1 && bitboard::get(mask, i / 2, j / 2) {
                debug_assert_eq!(grid[i][j], EMPTY_MARK);
                grid[i][j] = c;
            }
        }
    }
}

/// Print to stdout.
pub fn print(board: &bitboard::Board) {
    let (valid, _) = board.get_valid_mask();
    let mut grid = empty();
    write_mask(&mut grid, board.0, BLACK_MARK);
    write_mask(&mut grid, board.1, WHITE_MARK);
    write_mask(&mut grid, valid, VALID_MOVE_MARK);
    for row in grid.iter() {
        println!("{}", row.iter().collect::<String>());
    }
}
