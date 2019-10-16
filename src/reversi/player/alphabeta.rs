// todo: move thise to reversi
use reversi::bitboard::*;
use reversi::player::*;
use reversi::{H, W};

/// Alpha-beta 探索を行うプレイヤーです．
pub struct AlphaBetaSearchPlayer {
    rand: rand::Xor128,
}

impl AlphaBetaSearchPlayer {
    pub fn new(seed: u32) -> AlphaBetaSearchPlayer {
        AlphaBetaSearchPlayer {
            rand: rand::Xor128::from_seed(seed),
        }
    }

    fn search(&mut self, board: &Board, alpha: i32, beta: i32, depth: usize, passed: bool) -> i32 {
        debug_assert!(alpha <= beta);
        let (black_moves, parts) = board.get_valid_mask();
        let (white_moves, _) = board.switch().get_valid_mask();
        if depth == 0 || (black_moves == 0 && passed) {
            Self::evaluate(board, &(black_moves, white_moves))
        } else if black_moves == 0 {
            // pass
            -self.search(&board.switch(), -beta, -alpha, depth, true)
        } else {
            let mut alpha = alpha;
            // enumerate moves and shuffle them
            let mut moves = (0..H * W)
                .map(|i| 1 << i)
                .filter(|&mov| mov & black_moves == mov)
                .collect::<Vec<_>>();
            let n = moves.len();
            for i in 0..n - 1 {
                moves.swap(i, i + self.rand.next() as usize % (n - i));
            }

            for &mov in moves.iter() {
                let reversed = board.reverse_with_parts(mov, &parts);
                let score = -self.search(&reversed.switch(), -beta, -alpha, depth - 1, false);
                alpha = max(alpha, score);
                if alpha >= beta {
                    break;
                }
            }
            alpha
        }
    }

    /// 盤面を評価します．値が大きいほど優勢です．
    /// 序盤は石が置かれている場所が良いマスなら正の点数を，悪いマスなら負の点数を与えて和を返します．
    /// 終盤は黒石の数から白石の数を引いた値を返します．
    #[inline]
    fn evaluate(board: &Board, moves: &(Mask, Mask)) -> i32 {
        let Board(black_disks, white_disks) = *board;
        let (black_moves, white_moves) = *moves;
        if white_disks == 0 {
            INF
        } else if black_disks == 0 {
            -INF
        } else if (!(black_disks | white_disks)).count_ones() >= 10 {
            #[inline]
            fn eval(disks: Mask, moves: Mask) -> i32 {
                const ADD30: Mask =
                    0b_10000001_00000000_00000000_00000000_00000000_00000000_00000000_10000001;
                const SUB01: Mask =
                    0b_00011000_00000000_00011000_10111101_10111101_00011000_00000000_00011000;
                const SUB03: Mask =
                    0b_00000000_00111100_01000010_01000010_01000010_01000010_00111100_00000000;
                const SUB12: Mask =
                    0b_01000010_10000001_00000000_00000000_00000000_00000000_10000000_01000010;
                const SUB15: Mask =
                    0b_00000000_01000010_00000000_00000000_00000000_00000000_01000010_00000000;
                let mut weighted_disks = 0;
                weighted_disks += ((ADD30 & disks).count_ones() << 5) as i32;
                weighted_disks -= ((SUB01 & disks).count_ones() << 0) as i32;
                weighted_disks -= ((SUB03 & disks).count_ones() << 2) as i32;
                weighted_disks -= ((SUB12 & disks).count_ones() << 3) as i32;
                weighted_disks -= ((SUB15 & disks).count_ones() << 4) as i32;

                let num_moves = moves.count_ones() as i32;
                weighted_disks * 10 + num_moves * 5
            }
            eval(black_disks, black_moves) - eval(white_disks, white_moves)
        } else {
            black_disks.count_ones() as i32 - white_disks.count_ones() as i32
        }
    }
}

impl Player for AlphaBetaSearchPlayer {
    fn next(&mut self, board: &Board) -> Option<Mask> {
        let (black_moves, parts) = board.get_valid_mask();
        if black_moves == 0 {
            None
        } else {
            let mut best = (i32::min_value(), u32::min_value(), 0); // score, rand, position
            for mov in (0..H * W).map(|i| 1 << i).filter(|&m| black_moves & m == m) {
                let revered = board.reverse_with_parts(mov, &parts);
                let score = -self.search(&revered.switch(), -INF, INF, SEARCH_DEPTH, false);
                best = max(best, (score, self.rand.next() + 1, mov));
            }
            let (_, _, best_position) = best;
            Some(best_position)
        }
    }

    fn name(&self) -> &'static str {
        "Alpha-Beta"
    }
}
