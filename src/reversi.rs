//! リバーシのプレイヤープログラム (AI) です．
//! 簡単な評価関数を用いた alpha-beta 探索アルゴリズムを実装しています．

use std::cmp::max;
use std::collections::BTreeMap;
use std::io::{stdin, stdout, Write};

/// Xor-shift 乱数生成アルゴリズムにより乱数を生成します．
struct Xor128 {
    x: u32,
    y: u32,
    z: u32,
    w: u32,
}

impl Xor128 {
    fn from_seed(seed: u32) -> Xor128 {
        let mut res = Xor128 {
            x: 123456789,
            y: 987654321,
            z: 1000000007,
            w: seed,
        };
        for _ in 0..16 {
            res.next();
        }
        res
    }

    /// 内部状態を 1 ステップ進め，乱数を返します．
    fn next(&mut self) -> u32 {
        let t = self.x ^ (self.x << 11);
        self.x = self.y;
        self.y = self.z;
        self.z = self.w;
        self.w = (self.w ^ (self.w >> 19)) ^ (t ^ (t >> 8));
        self.w & 0x7FFFFFFF
    }
}

/// 標準入力から 1 行読み込み，空白文字を除いた先頭の文字を返します．
pub fn read_one_char() -> Option<char> {
    let mut line = String::new();
    stdin()
        .read_line(&mut line)
        .expect("failed to read from stdin");
    line.trim().chars().next()
}

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

/// 盤面全体のマスクを表す型です．`i` 行 `j` 列目のマスと `W` * `i` + `j` 番目のビットが対応します．
type Mask = u64;

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

/// 盤面の高さを表す定数です．
const H: usize = 8;
/// 盤面の幅を表す定数です．
const W: usize = 8;
/// 十分に大きな値を表す定数です．
const INF: i32 = 100_000_000;

/// 盤面を表す型です．
///
/// `self.0` が黒を表します．
/// `self.0` の `i` 番目のビットが立っているとき，かつそのときに限り，
/// 黒石が (`i` / `H`, `i` %  `H`) に存在することを表します．
/// `self.1` も同様に白石を表します．
#[derive(Clone)]
pub struct Board(Mask, Mask); // black, white

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
    fn new() -> Board {
        let black = put(put(0, 3, 4), 4, 3);
        let white = put(put(0, 3, 3), 4, 4);
        Board(black, white)
    }

    /// 反時計回りに回転した盤面を返します．
    fn rotate(&self) -> Board {
        Board(rotate_mask(self.0), rotate_mask(self.1))
    }

    /// 黒番と白番を入れ替えた盤面を返します．
    fn switch(&self) -> Board {
        Board(self.1, self.0)
    }

    /// 黒石の数と白石の数を返します．
    #[inline]
    fn count(&self) -> (u32, u32) {
        let Board(black, white) = *self;
        (black.count_ones(), white.count_ones())
    }

    /// どちらかの手番が石を置くことができるなら `true`，終了盤面であれば `false` を返します．
    fn continues(&self) -> bool {
        let (a, _) = self.get_valid_mask();
        let (b, _) = self.switch().get_valid_mask();
        a != 0 || b != 0
    }

    /// 黒石を置くことができる位置を調べます．
    ///
    /// 戻り値の `Mask` は置くことができる位置を表すビットマスクです．
    #[inline]
    fn get_valid_mask(&self) -> (Mask, ValidMaskParts) {
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
        fn get_rev(board: &Board, m: Mask, valid: Mask, transfer: &Fn(Mask) -> Mask) -> Mask {
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
    fn reverse(&self, mov: Mask) -> Board {
        let (_, parts) = self.get_valid_mask();
        self.reverse_with_parts(mov, &parts)
    }

    /// `mov` が表す位置に黒石を置いて裏返した後の盤面を返します．
    #[inline]
    fn reverse_with_parts(&self, mov: Mask, parts: &ValidMaskParts) -> Board {
        let reverse = self.get_reverse_mask(&parts, mov);
        self.reverse_by_mask(mov, reverse)
    }

    /// `mov` が表す位置に黒石を置いて裏返した後の盤面を返します．
    #[inline]
    fn reverse_by_mask(&self, mov: Mask, rev: Mask) -> Board {
        Board(self.0 | mov | rev, self.1 ^ rev)
    }

    /// 標準出力に出力します．
    fn print(&self) {
        let (valid, _) = self.get_valid_mask();
        let mut g = empty_grid();
        write_mask_to(&mut g, self.0, 'X');
        write_mask_to(&mut g, self.1, 'O');
        write_mask_to(&mut g, valid, '.');
        for row in g.iter() {
            println!("{}", row.iter().collect::<String>());
        }
    }
}

/// 反時計回りに90度回転します．
#[inline]
fn rotate_mask(x: Mask) -> Mask {
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
fn put(mask: Mask, r: usize, c: usize) -> Mask {
    debug_assert!(!get(mask, r, c));
    debug_assert!(r < H);
    debug_assert!(c < W);
    mask | (1 << (r * 8 + c))
}

/// (`r`, `c`) に対応する位置のビットを取得します．
#[inline]
fn get(mask: Mask, r: usize, c: usize) -> bool {
    debug_assert!(r < H);
    debug_assert!(c < W);
    mask >> (r * 8 + c) & 1 == 1
}

/// (`r`, `c`) に対応する位置のビットのみが立った盤面を返します．
fn position_to_mask(r: usize, c: usize) -> Mask {
    debug_assert!(r < H);
    debug_assert!(c < W);
    put(0, r, c)
}

/// ちょうど 1 箇所だけ立ったマスクから，その場所の座標を求めます．
fn movemask_to_position(mask: Mask) -> (usize, usize) {
    let n = mask.trailing_zeros() as usize;
    (n / H, n % H)
}

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

/// Alpha-beta 探索を行うプレイヤーです．
pub struct AlphaBetaSearchPlayer {
    rand: Xor128,
}

impl AlphaBetaSearchPlayer {
    pub fn new(seed: u32) -> AlphaBetaSearchPlayer {
        AlphaBetaSearchPlayer {
            rand: Xor128::from_seed(seed),
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

/// ランダムな行動をするプレイヤーです．
pub struct RandomPlayer {
    rand: Xor128,
}

impl RandomPlayer {
    pub fn new(seed: u32) -> RandomPlayer {
        RandomPlayer {
            rand: Xor128::from_seed(seed),
        }
    }
}

impl Player for RandomPlayer {
    fn next(&mut self, board: &Board) -> Option<Mask> {
        let (black_moves, _) = board.get_valid_mask();
        // board.print();
        // println!("{:064b}", black_moves);
        if black_moves == 0 {
            None
        } else {
            let mut best = (u32::min_value(), 0);
            for mov in (0..H * W).map(|i| 1 << i).filter(|&m| black_moves & m == m) {
                best = max(best, (self.rand.next() + 1, mov));
            }
            let (_, best_position) = best;
            debug_assert!(best_position != 0);
            Some(best_position)
        }
    }

    fn name(&self) -> &'static str {
        "Random"
    }
}

/// 人間がキーボードから手を入力するのを補助します．
pub struct HumanPlayer;

impl HumanPlayer {
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
                    println!("  {} : {}", k, position_to_name(r, c));
                }
                print!(
                    "Type any character of [{}]: ",
                    cand.iter().collect::<String>()
                );
                stdout().flush().unwrap();
                c = read_one_char();
            }
            let (r, c) = map[&c.unwrap()];
            Some(position_to_mask(r, c))
        }
    }

    fn name(&self) -> &'static str {
        "Human"
    }
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
    black: Box<Player>,
    white: Box<Player>,
    board: Board,
    next_player: Turn,
    pub result: Option<GameResult>,
    pub verbose: bool,
}

impl GameManager {
    pub fn new(black: Box<Player>, white: Box<Player>) -> GameManager {
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
            result.board.print();
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
