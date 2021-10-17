use reversi::{H, W};

// Bitmask of the 8 x 8 board. The cell on i-th row of j-th column is corresponds to the (W*i+j)-th bit.
pub type Mask = u64;

/// State of the board.
/// self.0 represents the places of black disks.
/// The i-th bit of self.0 is positive (one) iff. cell (i/H, i%H) has a black disk.
/// The same for self.1 for white.
#[derive(Clone)]
pub struct Board(pub Mask, pub Mask); // black, white

/// ValidMaskParts represents the cells where a disk can be placed one of there.
/// Consider a bitmask which we apply `rotate` for i times to ValidMaskParts[i].
/// Then it represents places where we can flip at least one white disk to be black by putting black there.
type ValidMaskParts = [(Mask, Mask); 4];

impl Board {
    // Returns the begging of games with four disks.
    pub fn new() -> Board {
        let black = put(put(0, 3, 4), 4, 3);
        let white = put(put(0, 3, 3), 4, 4);
        Board(black, white)
    }

    /// Returns board rotated ccw.
    fn rotate(&self) -> Board {
        Board(rotate_mask(self.0), rotate_mask(self.1))
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
    #[inline]
    pub fn get_valid_mask(&self) -> (Mask, ValidMaskParts) {
        // Return the places that can flip white disks alining straight from right to left.
        #[inline]
        fn valid_mask_left(board: &Board) -> Mask {
            let Board(black, white) = *board;
            let w = white & 0x7e7e7e7e7e7e7e7e;
            let t = w & (black >> 1);
            let t = t | w & (t >> 1);
            let t = t | w & (t >> 1);
            let t = t | w & (t >> 1);
            let t = t | w & (t >> 1);
            let t = t | w & (t >> 1);
            let blank = !(black | white);
            blank & (t >> 1)
        }

        // Return the places that can flip white disks alining straight from bottom-right to top-left.
        #[inline]
        fn valid_mask_top_left(board: &Board) -> Mask {
            let Board(black, white) = *board;
            let w = white & 0x007e7e7e7e7e7e00;
            let t = w & (black >> 9);
            let t = t | w & (t >> 9);
            let t = t | w & (t >> 9);
            let t = t | w & (t >> 9);
            let t = t | w & (t >> 9);
            let t = t | w & (t >> 9);
            let blank = !(black | white);
            return blank & (t >> 9);
        }

        // Calculate the mask for valid cells of left direction or top-left direction four times
        // with rotating the board and taking bitwise-or.
        // It also returns partial (rotated) masks of each step to be used in the next step.
        let mut acc = 0;
        let mut res = [(0, 0); 4];
        let mut rotated_board = self.clone();
        for r in res.iter_mut() {
            let left = valid_mask_left(&rotated_board);
            let top_left = valid_mask_top_left(&rotated_board);
            *r = (left, top_left);
            acc |= left | top_left;
            rotated_board = rotated_board.rotate();
            acc = rotate_mask(acc);
        }
        (acc, res)
    }

    /// Returns the mask of white disks that will be flipped
    /// when we put a black disk at the `mov`.
    /// `mov` must have just one positive bit.
    fn get_flip_mask(&self, parts: &ValidMaskParts, mov: Mask) -> Mask {
        debug_assert!((self.0 | self.1) & mov == 0);
        debug_assert_eq!(mov.count_ones(), 1);

        // Moves all disks to left direction.
        #[inline]
        fn transfer_to_left(m: Mask) -> Mask {
            (m << 1) & 0xfefefefefefefefe
        }

        // Moves all disks to top-left direction.
        #[inline]
        fn transfer_to_top_left(m: Mask) -> Mask {
            (m << 9) & 0xfefefefefefefe00
        }

        // Returns flipped white disks aligning by direction of `transfer` when a disk is put on `mov`.
        // `mov` must have just one positive bit.
        // `mov` is allowed to be invalid (impossible to flip any disks)
        // since this function is called for all of four rotations.
        #[inline]
        fn get_flip_mask(
            board: &Board,
            mov: Mask,
            valid: Mask,
            transfer: &dyn Fn(Mask) -> Mask,
        ) -> Mask {
            let Board(black, white) = *board;
            if (valid & mov) == mov {
                // mov is a subset of valid, so Will flip some disks
                // Shift mov disk by one cell.
                let mut mov = transfer(mov);
                // Walk through white disks until mov hits to the opposite.
                let mut rev = 0;
                while mov != 0 && (mov & white) != 0 {
                    rev |= mov;
                    mov = transfer(mov);
                }
                if (mov & black) == 0 {
                    // The disk of other side not found, cannot flip any white.
                    0
                } else {
                    // Reached to black disk put at this turn.
                    rev
                }
            } else {
                // Cannot flip any disks
                0
            }
        }

        // Calculate the mask for flipped cells of left direction and top-left direction four times
        // with rotating the board and taking bitwise-or.
        // It also returns partial (rotated) masks of each step to be used in the next step.
        let mut res = 0;
        let mut rotated_board = self.clone();
        let mut mov = mov;
        for &(valid_left, valid_top_left) in parts.iter() {
            res |= get_flip_mask(&rotated_board, mov, valid_left, &transfer_to_left)
                | get_flip_mask(&rotated_board, mov, valid_top_left, &transfer_to_top_left);
            res = rotate_mask(res);
            rotated_board = rotated_board.rotate();
            mov = rotate_mask(mov);
        }

        debug_assert!(res != 0);
        res
    }

    /// Returns the mask after we put a black disk at `mov` then some white disks are flipped.
    #[inline]
    pub fn flip(&self, mov: Mask) -> Board {
        let (_, hints) = self.get_valid_mask();
        self.flip_with_hints(mov, &hints)
    }

    /// Returns the board after we put a black disk at `mov` then flip white disks.
    /// `hints` are information for working in each direction.
    #[inline]
    pub fn flip_with_hints(&self, mov: Mask, hits: &ValidMaskParts) -> Board {
        let flip = self.get_flip_mask(&hits, mov);
        Board(self.0 | mov | flip, self.1 ^ flip)
    }
}

/// Rotate 90 degrees by ccw.
#[inline]
pub fn rotate_mask(x: Mask) -> Mask {
    let x = ((x << 1) & 0xAA00AA00AA00AA00)
        | ((x >> 1) & 0x0055005500550055)
        | ((x >> 8) & 0x00AA00AA00AA00AA)
        | ((x << 8) & 0x5500550055005500);
    let x = ((x << 2) & 0xCCCC0000CCCC0000)
        | ((x >> 2) & 0x0000333300003333)
        | ((x >> 16) & 0x0000CCCC0000CCCC)
        | ((x << 16) & 0x3333000033330000);
    let x = ((x << 4) & 0xF0F0F0F000000000)
        | ((x >> 4) & 0x000000000F0F0F0F)
        | ((x >> 32) & 0x00000000F0F0F0F0)
        | ((x << 32) & 0x0F0F0F0F00000000);
    x
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
