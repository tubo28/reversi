use reversi::{H, W};

/// 盤面全体のマスクを表す型です．`i` 行 `j` 列目のマスと `W` * `i` + `j` 番目のビットが対応します．
pub type Mask = u64;

/// 盤面を表す型です．
///
/// `self.0` が黒を表します．
/// `self.0` の `i` 番目のビットが立っているとき，かつそのときに限り，
/// 黒石が (`i` / `H`, `i` %  `H`) に存在することを表します．
/// `self.1` も同様に白石を表します．
#[derive(Clone)]
pub struct Board(pub Mask, pub Mask); // black, white

/// 石を置くことができる位置を計算するためのヒントを表す型です．
///
/// `i` 番目の要素は，
/// `i` 回 `rotate` を実行したあとの盤面に対して黒を置くことで白が取れる場所を表します．
///
/// `i` 番目の要素が `(a, b)` であると仮定します．
/// `a` においてビットが立っている位置に関して，
/// その位置のまっすぐ右に黒石が存在し，その間は全て白石が存在します．
/// また，`b` においてビットが立っている位置に関して，
/// その位置のまっすぐ右下に黒石が存在し，その間は全て白石が存在します．
type ValidMaskParts = [(Mask, Mask); 4];

impl Board {
    pub fn new() -> Board {
        let black = put(put(0, 3, 4), 4, 3);
        let white = put(put(0, 3, 3), 4, 4);
        Board(black, white)
    }

    /// 反時計回りに回転した盤面を返します．
    fn rotate(&self) -> Board {
        Board(rotate_mask(self.0), rotate_mask(self.1))
    }

    /// 黒番と白番を入れ替えた盤面を返します．
    pub fn switch(&self) -> Board {
        Board(self.1, self.0)
    }

    /// 黒石の数と白石の数を返します．
    #[inline]
    pub fn count(&self) -> (u32, u32) {
        let Board(black, white) = *self;
        (black.count_ones(), white.count_ones())
    }

    /// どちらかの手番が石を置くことができるなら `true`，終了盤面であれば `false` を返します．
    pub fn continues(&self) -> bool {
        let (a, _) = self.get_valid_mask();
        let (b, _) = self.switch().get_valid_mask();
        a != 0 || b != 0
    }

    /// 黒石を置くことができる位置を調べます．
    ///
    /// 戻り値の `Mask` は置くことができる位置を表すビットマスクです．
    #[inline]
    pub fn get_valid_mask(&self) -> (Mask, ValidMaskParts) {
        #[inline]
        fn valid_mask_from_left(board: &Board) -> Mask {
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

        #[inline]
        fn valid_mask_from_top_left(board: &Board) -> Mask {
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

        let mut acc = 0;
        let mut res = [(0, 0); 4];
        let mut rotated_board = self.clone();
        for r in res.iter_mut() {
            let left = valid_mask_from_left(&rotated_board);
            let top_left = valid_mask_from_top_left(&rotated_board);
            *r = (left, top_left);
            acc |= left | top_left;
            rotated_board = rotated_board.rotate();
            acc = rotate_mask(acc);
        }
        (acc, res)
    }

    /// `mov` が表す位置に黒をおいたときに裏返る白石のビットマスクを返します．
    /// `mov` の立っている 1 の数はちょうど 1 でなければいけません．
    fn get_reverse_mask(&self, parts: &ValidMaskParts, mov: Mask) -> Mask {
        debug_assert!((self.0 | self.1) & mov == 0);
        debug_assert_eq!(mov.count_ones(), 1);

        #[inline]
        fn transfer_to_left(m: Mask) -> Mask {
            (m << 1) & 0xfefefefefefefefe
        }

        #[inline]
        fn transfer_to_top_left(m: Mask) -> Mask {
            (m << 9) & 0xfefefefefefefe00
        }

        #[inline]
        fn get_rev(board: &Board, m: Mask, valid: Mask, transfer: &dyn Fn(Mask) -> Mask) -> Mask {
            let Board(black, white) = *board;
            if (valid & m) == m {
                let mut rev = 0;
                let mut mask = transfer(m);
                while mask != 0 && (mask & white) != 0 {
                    rev |= mask;
                    mask = transfer(mask);
                }
                if (mask & black) == 0 {
                    0
                } else {
                    rev
                }
            } else {
                0
            }
        }

        let mut res = 0;
        let mut rotated_board = self.clone();
        let mut mov = mov;

        for &(valid_left, valid_top_left) in parts.iter() {
            res |= get_rev(&rotated_board, mov, valid_left, &transfer_to_left)
                | get_rev(&rotated_board, mov, valid_top_left, &transfer_to_top_left);
            res = rotate_mask(res);
            rotated_board = rotated_board.rotate();
            mov = rotate_mask(mov);
        }

        debug_assert!(res != 0);
        res
    }

    /// `mov` が表す位置に黒石を置いて裏返した後の盤面を返します．
    #[inline]
    pub fn reverse(&self, mov: Mask) -> Board {
        let (_, parts) = self.get_valid_mask();
        self.reverse_with_parts(mov, &parts)
    }

    /// `mov` が表す位置に黒石を置いて裏返した後の盤面を返します．
    #[inline]
    pub fn reverse_with_parts(&self, mov: Mask, parts: &ValidMaskParts) -> Board {
        let reverse = self.get_reverse_mask(&parts, mov);
        self.reverse_by_mask(mov, reverse)
    }

    /// `mov` が表す位置に黒石を置いて裏返した後の盤面を返します．
    #[inline]
    fn reverse_by_mask(&self, mov: Mask, rev: Mask) -> Board {
        Board(self.0 | mov | rev, self.1 ^ rev)
    }
}

/// 反時計回りに90度回転します．
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

/// (`r`, `c`) に対応する位置のビットが立った盤面を返します．
#[inline]
pub fn put(mask: Mask, r: usize, c: usize) -> Mask {
    debug_assert!(!get(mask, r, c));
    debug_assert!(r < H);
    debug_assert!(c < W);
    mask | (1 << (r * 8 + c))
}

/// (`r`, `c`) に対応する位置のビットを取得します．
#[inline]
pub fn get(mask: Mask, r: usize, c: usize) -> bool {
    debug_assert!(r < H);
    debug_assert!(c < W);
    mask >> (r * 8 + c) & 1 == 1
}

/// (`r`, `c`) に対応する位置のビットのみが立った盤面を返します．
pub fn position_to_mask(r: usize, c: usize) -> Mask {
    debug_assert!(r < H);
    debug_assert!(c < W);
    put(0, r, c)
}

/// ちょうど 1 箇所だけ立ったマスクから，その場所の座標を求めます．
pub fn movemask_to_position(mask: Mask) -> (usize, usize) {
    let n = mask.trailing_zeros() as usize;
    (n / H, n % H)
}
