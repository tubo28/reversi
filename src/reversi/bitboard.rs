use crate::reversi::{H, W};

// Bitmask of the 8 x 8 board. The cell on (i, j) (i-th row of j-th column)
// corresponds to the (W*i+j)-th bit.
pub type Mask = u64;

/// State of the board.
/// self.0 represents the places of black disks.
/// The i-th bit of self.0 is positive (one) iff. cell (i/H, i%H) has a black disk.
/// The same for self.1 for white.
#[derive(Clone)]
pub struct Board(pub Mask, pub Mask); // black, white

/// Vestigial: this used to carry the per-rotation partial masks consumed by the
/// old rotation-based flip. The flip is now computed directly (see `flip_disks`),
/// so `get_valid_mask` returns a dummy of this type and `flip_with_hints` ignores
/// it. Kept only so existing callers threading it through still compile.
type ValidMaskParts = [(Mask, Mask); 4];

impl Board {
    // Returns the begging of games with four disks.
    pub fn new() -> Board {
        let black = put(put(0, 3, 4), 4, 3);
        let white = put(put(0, 3, 3), 4, 4);
        Board(black, white)
    }

    /// Returns board with black and white swapped.
    pub fn switch(&self) -> Board {
        Board(self.1, self.0)
    }

    /// Returns the number of disks of black and white.
    #[inline]
    pub fn count(&self) -> (u32, u32) {
        // usize?
        let Board(black, white) = *self;
        (black.count_ones(), white.count_ones())
    }

    /// Returns true iff. either players can place a piece.
    /// False means the end of this game.
    pub fn continues(&self) -> bool {
        let (a, _) = self.get_valid_mask();
        let (b, _) = self.switch().get_valid_mask();
        a != 0 || b != 0
    }

    /// Calculates cells in which we can put a black disk.
    ///
    /// The second tuple element (`ValidMaskParts`) is retained only for backward
    /// compatibility with `flip_with_hints`; it is no longer used internally (the
    /// flip is now computed directly from the board), so a dummy is returned.
    #[inline]
    pub fn get_valid_mask(&self) -> (Mask, ValidMaskParts) {
        (legal_moves(self.0, self.1), [(0, 0); 4])
    }

    /// Returns the mask after we put a black disk at `mov` then some white disks
    /// are flipped.
    #[inline]
    pub fn flip(&self, mov: Mask) -> Board {
        let flip = flip_disks(self.0, self.1, mov);
        Board(self.0 | mov | flip, self.1 ^ flip)
    }

    /// Same as `flip`. The `hints` argument is ignored — kept only so existing
    /// callers that threaded `ValidMaskParts` through do not need to change.
    #[inline]
    pub fn flip_with_hints(&self, mov: Mask, _hints: &ValidMaskParts) -> Board {
        self.flip(mov)
    }
}

/// Interior propagator mask: both file A (col 0) and file H (col 7) cleared, so
/// that horizontal and diagonal bit shifts cannot wrap across a row boundary.
const NOT_EDGE_HORIZ: Mask = 0x7e7e7e7e7e7e7e7e;

/// Legal moves for black (black to move), computed with a branchless
/// 8-direction parallel-prefix ("smear") fill — no board rotation, no closures.
///
/// For each direction we walk up to six opponent disks starting from our own
/// disks (`t`), then a legal move is an empty cell one step beyond that run.
/// The `<<`/`>>` shift amount encodes the direction on the `row*8 + col` layout
/// (±1 = E/W, ±8 = N/S, ±9 = NW/SE, ±7 = NE/SW); the propagator is the interior
/// `white` for horizontal-crossing directions and the full `white` for vertical.
#[inline]
pub fn legal_moves(black: Mask, white: Mask) -> Mask {
    let empty = !(black | white);
    let h = white & NOT_EDGE_HORIZ; // horizontal / diagonal (row-wrap guarded)
    let v = white; // vertical (a shift by 8 cannot wrap)

    macro_rules! dir {
        ($prop:expr, $op:tt, $s:expr) => {{
            let p = $prop;
            let mut t = p & (black $op $s);
            t |= p & (t $op $s);
            t |= p & (t $op $s);
            t |= p & (t $op $s);
            t |= p & (t $op $s);
            t |= p & (t $op $s);
            empty & (t $op $s)
        }};
    }

    dir!(h, <<, 1) // E
        | dir!(h, >>, 1) // W
        | dir!(v, <<, 8) // S
        | dir!(v, >>, 8) // N
        | dir!(h, <<, 9) // SE
        | dir!(h, >>, 9) // NW
        | dir!(h, <<, 7) // SW
        | dir!(h, >>, 7) // NE
}

/// White disks flipped when black plays at `mov` (which must be a single empty
/// cell). Same 8-direction smear as `legal_moves`, but here each direction keeps
/// the opponent run `t` iff the cell beyond it is one of our own disks (`black`),
/// i.e. the run is flanked. Branchless: the run is masked in or out by whether a
/// flanking disk exists. No `dyn` dispatch, no data-dependent loop.
#[inline]
pub fn flip_disks(black: Mask, white: Mask, mov: Mask) -> Mask {
    let h = white & NOT_EDGE_HORIZ;
    let v = white;

    macro_rules! dir {
        ($prop:expr, $op:tt, $s:expr) => {{
            let p = $prop;
            let mut t = p & (mov $op $s);
            t |= p & (t $op $s);
            t |= p & (t $op $s);
            t |= p & (t $op $s);
            t |= p & (t $op $s);
            t |= p & (t $op $s);
            // Keep the whole run only if it is flanked by one of our own disks.
            let flanked = ((t $op $s) & black != 0) as u64;
            t & flanked.wrapping_neg()
        }};
    }

    dir!(h, <<, 1)
        | dir!(h, >>, 1)
        | dir!(v, <<, 8)
        | dir!(v, >>, 8)
        | dir!(h, <<, 9)
        | dir!(h, >>, 9)
        | dir!(h, <<, 7)
        | dir!(h, >>, 7)
}

/// Put disk in the cell at (r, c) cell.
#[inline]
pub fn put(mask: Mask, r: usize, c: usize) -> Mask {
    debug_assert!(!get(mask, r, c));
    debug_assert!(r < H);
    debug_assert!(c < W);
    mask | (1 << (r * 8 + c))
}

/// Check if the cell at cell (r, c) has a disk.
#[inline]
pub fn get(mask: Mask, r: usize, c: usize) -> bool {
    debug_assert!(r < H);
    debug_assert!(c < W);
    mask >> (r * 8 + c) & 1 == 1
}

/// Returns the mask that cell in (r, c) only has a disk.
pub fn position_to_mask(r: usize, c: usize) -> Mask {
    debug_assert!(r < H);
    debug_assert!(c < W);
    put(0, r, c)
}

/// Returns the coordinate of the disk put in mask
/// which is lexicographically smallest by the coordinate (r, c).
/// Usually used for find the disk put on a board which has only one disk.
pub fn coordinate(mask: Mask) -> (usize, usize) {
    let n = mask.trailing_zeros() as usize;
    (n / H, n % H)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reversi::rand::Xor128;

    // The eight (dr, dc) step directions.
    const DIRS: [(i32, i32); 8] =
        [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)];

    #[inline]
    fn bit(mask: Mask, r: i32, c: i32) -> bool {
        mask >> (r * 8 + c) & 1 == 1
    }

    /// Obvious-by-inspection scalar oracle for the legal moves of black.
    fn legal_moves_ref(black: Mask, white: Mask) -> Mask {
        let occupied = black | white;
        let mut moves = 0;
        for r in 0..8 {
            for c in 0..8 {
                if occupied >> (r * 8 + c) & 1 == 1 {
                    continue; // cell is occupied
                }
                for &(dr, dc) in DIRS.iter() {
                    let (mut nr, mut nc) = (r + dr, c + dc);
                    let mut seen_opp = false;
                    loop {
                        if !(0..8).contains(&nr) || !(0..8).contains(&nc) {
                            break; // ran off the board without flanking
                        }
                        if bit(white, nr, nc) {
                            seen_opp = true;
                            nr += dr;
                            nc += dc;
                        } else if bit(black, nr, nc) {
                            if seen_opp {
                                moves |= 1u64 << (r * 8 + c);
                            }
                            break;
                        } else {
                            break; // empty
                        }
                    }
                }
            }
        }
        moves
    }

    /// Obvious-by-inspection scalar oracle for the disks flipped when black plays
    /// at `mov` (a single empty cell).
    fn flip_ref(black: Mask, white: Mask, mov: Mask) -> Mask {
        let idx = mov.trailing_zeros() as i32;
        let (r, c) = (idx / 8, idx % 8);
        let mut flipped = 0;
        for &(dr, dc) in DIRS.iter() {
            let (mut nr, mut nc) = (r + dr, c + dc);
            let mut run = 0u64;
            loop {
                if !(0..8).contains(&nr) || !(0..8).contains(&nc) {
                    run = 0; // off board: not flanked
                    break;
                }
                if bit(white, nr, nc) {
                    run |= 1u64 << (nr * 8 + nc);
                    nr += dr;
                    nc += dc;
                } else if bit(black, nr, nc) {
                    break; // flanked: keep run
                } else {
                    run = 0; // empty: not flanked
                    break;
                }
            }
            flipped |= run;
        }
        flipped
    }

    #[test]
    fn legal_moves_initial_board_matches_oracle() {
        let b = Board::new();
        let expected = (1u64 << 19) | (1u64 << 26) | (1u64 << 37) | (1u64 << 44); // the 4 opening moves
        assert_eq!(legal_moves(b.0, b.1), expected);
        assert_eq!(legal_moves(b.0, b.1), legal_moves_ref(b.0, b.1));
    }

    // Drives realistic reachable positions by random self-play, and at every
    // position cross-checks the bitboard routines against the obvious-by-
    // inspection scalar oracle, for the mover and each legal move.
    #[test]
    fn routines_match_oracle_over_random_playouts() {
        let mut rng = Xor128::from_seed(12345);
        let mut checked_positions = 0u32;
        for _ in 0..300u32 {
            let mut board = Board::new();
            let mut passed = false;
            loop {
                let moves = legal_moves(board.0, board.1);
                assert_eq!(moves, legal_moves_ref(board.0, board.1), "mobility vs oracle");

                if moves == 0 {
                    if passed {
                        break; // both sides passed: game over
                    }
                    board = board.switch();
                    passed = true;
                    continue;
                }
                passed = false;
                checked_positions += 1;

                // Cross-check flips for every legal move, and collect them.
                let mut choices: Vec<Mask> = Vec::new();
                let mut m = moves;
                while m != 0 {
                    let mov = m & m.wrapping_neg();
                    let flip = flip_disks(board.0, board.1, mov);
                    assert_eq!(flip, flip_ref(board.0, board.1, mov), "flip vs oracle");
                    choices.push(mov);
                    m &= m - 1;
                }

                // Play a random legal move and continue.
                let mov = choices[rng.next() as usize % choices.len()];
                board = board.flip(mov).switch();
            }
        }
        // Sanity: we actually exercised a large number of distinct positions.
        assert!(checked_positions > 5000, "too few positions checked: {checked_positions}");
    }
}
