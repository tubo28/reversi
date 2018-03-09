// use std;
use std::cmp::max;
use std::io::{stdin, stdout, Write};
use std::collections::BTreeMap;

pub struct Xor128 {
    x: u32,
    y: u32,
    z: u32,
    w: u32,
}

impl Xor128 {
    pub fn new() -> Xor128 {
        use std::io::Read;
        let mut buf = [0; 4];
        let mut urandom = std::fs::File::open("/dev/urandom").expect("failed opening /dev/urandom");
        urandom.read(&mut buf).expect("failed reading /dev/urandom");
        let seed = buf.iter().fold(0, |a, &x| a << 4 | x as u32);
        Xor128::from_seed(seed)
    }

    pub fn from_seed(seed: u32) -> Xor128 {
        let mut res = Xor128 {
            x: 123456789,
            y: 987654321,
            z: 1000000007,
            w: seed,
        };
        for _ in 0..16 {
            res.gen();
        }
        res
    }

    pub fn gen(&mut self) -> u32 {
        let t = self.x ^ (self.x << 11);
        self.x = self.y;
        self.y = self.z;
        self.z = self.w;
        self.w = (self.w ^ (self.w >> 19)) ^ (t ^ (t >> 8));
        self.w
    }

    pub fn sample<'a, T>(&mut self, values: &'a [T]) -> Option<&'a T>
    where
        Self: Sized,
    {
        if values.is_empty() {
            None
        } else {
            Some(&values[(self.gen() % values.len() as u32) as usize])
        }
    }
}

type Mask = u64;

const H: usize = 8;
const W: usize = 8;
const INF: i32 = 1_000_000;

/// 半時計回りに90度回転します．
#[inline]
fn rotate(x: Mask) -> Mask {
    let x = ((x << 1) & 0xAA00AA00AA00AA00) | ((x >> 1) & 0x0055005500550055)
        | ((x >> 8) & 0x00AA00AA00AA00AA) | ((x << 8) & 0x5500550055005500);

    let x = ((x << 2) & 0xCCCC0000CCCC0000) | ((x >> 2) & 0x0000333300003333)
        | ((x >> 16) & 0x0000CCCC0000CCCC) | ((x << 16) & 0x3333000033330000);

    let x = ((x << 4) & 0xF0F0F0F000000000) | ((x >> 4) & 0x000000000F0F0F0F)
        | ((x >> 32) & 0x00000000F0F0F0F0) | ((x << 32) & 0x0F0F0F0F00000000);
    x
}

/// 置くことができる位置を表すビットマスクを返します．
/// 戻り値の i 番目の要素は，black と white を i 回 rotate を実行したあとの盤面に対して
/// 黒を置くことで白が取れる場所を表します．
/// i 番目の要素が (a, b) であると仮定します．
/// a において 1 が立っているマスに黒を置くと，
/// その位置のまっすぐ右にある黒と挟むことができます．
/// また，b において 1 が立っているマスに黒を置くと，
/// その位置よりまっすぐ右下にある黒と挟むことができます．
#[inline]
fn get_valid_pos(black: Mask, white: Mask) -> (Mask, [(Mask, Mask); 4]) {
    #[inline]
    fn valid_mask_from_left(black: Mask, white: Mask) -> Mask {
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
    fn valid_mask_from_top_left(black: Mask, white: Mask) -> Mask {
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

    let mut or = 0;
    let mut res = [(0, 0); 4];
    let mut black = black;
    let mut white = white;
    for r in res.iter_mut() {
        let mut left = valid_mask_from_left(black, white);
        let mut top_left = valid_mask_from_top_left(black, white);
        *r = (left, top_left);
        or |= left | top_left;
        black = rotate(black);
        white = rotate(white);
        or = rotate(or);
    }
    (or, res)
}

/// 盤面のビットマスクと `get_valid_pos` の戻り値，一箇所のみ 1 が立っているマスクを与えると，
/// その位置に黒をおいたときに裏返る白石のビットマスクを返します．
/// 裏返すことができないなら `Err` を返します．
fn get_reverse_mask(
    black: Mask,
    white: Mask,
    valid: &[(Mask, Mask); 4],
    m: Mask,
) -> Result<Mask, ()> {
    fn transfer_left(m: Mask) -> Mask {
        (m << 1) & 0xfefefefefefefefe
    }

    fn transfer_top_left(m: Mask) -> Mask {
        (m << 9) & 0xfefefefefefefe00
    }

    if ((black | white) & m) != 0 {
        return Err(());
    }

    let mut res = 0;

    let mut black = black;
    let mut white = white;
    let mut m = m;

    for &(valid_left, valid_top_left) in valid.iter() {
        let rev_left = if (valid_left & m) == m {
            let mut rev = 0;
            let mut mask = transfer_left(m);
            while mask != 0 && (mask & white) != 0 {
                rev |= mask;
                mask = transfer_left(mask);
            }
            if (mask & black) == 0 {
                0
            } else {
                rev
            }
        } else {
            0
        };

        let rev_top_left = if (valid_top_left & m) == m {
            let mut rev = 0;
            let mut mask = transfer_top_left(m);
            while mask != 0 && (mask & white) != 0 {
                rev |= mask;
                mask = transfer_top_left(mask);
            }
            if (mask & black) == 0 {
                0
            } else {
                rev
            }
        } else {
            0
        };

        res |= rev_left | rev_top_left;

        res = rotate(res);
        black = rotate(black);
        white = rotate(white);
        m = rotate(m);
    }

    if res == 0 {
        Err(())
    } else {
        Ok(res)
    }
}

/// 初期盤面を返します．
fn initial_state() -> (Mask, Mask) {
    let black = put(put(0, 3, 3), 4, 4);
    let white = put(put(0, 3, 4), 4, 3);
    (black, white)
}

#[inline]
fn put(mask: Mask, r: usize, c: usize) -> Mask {
    debug_assert!(get(mask, r, c));
    mask | (1u64 << (r * 8 + c))
}

#[inline]
fn get(mask: Mask, r: usize, c: usize) -> bool {
    mask >> (r * 8 + c) & 1 == 1
}

fn pos_to_mask(r: usize, c: usize) -> Mask {
    put(0, r, c)
}

fn write_mask(g: &mut Vec<Vec<char>>, mask: Mask, c: char) {
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

fn write_grid(g: &mut Vec<Vec<char>>) {
    for i in 0..H * 2 + 1 {
        for j in 0..W * 2 + 1 {
            if i % 2 != 1 || j % 2 != 1 {
                let c = if i % 2 == 0 && j % 2 == 0 {
                    '+'
                } else if i % 2 == 0 {
                    '-'
                } else {
                    '|'
                };
                debug_assert_eq!(g[i][j], ' ');
                g[i][j] = c;
            }
        }
    }
}

fn write_board(g: &mut Vec<Vec<char>>, black: Mask, white: Mask) {
    write_mask(g, black, '@');
    write_mask(g, white, 'O');
    write_grid(g);
}

fn print_board(black: Mask, white: Mask) {
    print_board_with_indent(black, white, 0)
}

fn print_board_with_indent(black: Mask, white: Mask, indent: usize) {
    let (valid, _) = get_valid_pos(black, white);
    let mut g = vec![vec![' '; H * 2 + 1]; W * 2 + 1];
    write_board(&mut g, black, white);
    write_mask(&mut g, valid, '.');
    for row in g.iter() {
        let indent = (0..indent).map(|_| ' ').collect::<String>();
        println!("{}{}", indent, row.iter().collect::<String>());
    }
}

fn print_mask(mask: Mask) {
    let mut g = vec![vec![' '; H * 2 + 1]; W * 2 + 1];
    write_mask(&mut g, mask, 'O');
    write_grid(&mut g);
    for row in g.iter() {
        println!("{}", row.iter().collect::<String>());
    }
}

fn alpha_beta(black: Mask, white: Mask, alpha: i32, beta: i32, passed: bool, depth: usize) -> i32 {
    if true {
        // let indent = (0..(10-depth)*2).map(|_| ' ').collect::<String>();
        // print!("{}", indent);
        // println!("{} {} {}", alpha, beta, depth);
        // print_board(black, white, (5 - depth) * 2);
    }

    let (valid_mask, valid) = get_valid_pos(black, white);

    if depth == 0 || (valid_mask == 0 && passed) {
        let score = black.count_ones() as i32 - white.count_ones() as i32;
        // print!("{} ", score);
        // println!("b {} w {}", black.count_ones(), white.count_ones());
        score
    } else if valid_mask == 0 {
        -alpha_beta(white, black, -beta, -alpha, true, depth - 1)
    } else {
        let mut alpha = alpha;
        for i in 0..64 {
            let mov = 1 << i;
            if valid_mask & mov == mov {
                let rev = get_reverse_mask(black, white, &valid, mov).expect("err");
                let black = black | mov | rev;
                let white = white ^ rev;
                let score = alpha_beta(white, black, -beta, -alpha, false, depth - 1);
                alpha = max(alpha, -score);
                if alpha >= beta {
                    return alpha;
                }
            }
        }
        // print!("{} ", alpha);
        alpha
    }
}

fn simple_search(black: Mask, white: Mask, depth: usize) -> i32 {
    if true {
        // let indent = (0..(10-depth)*2).map(|_| ' ').collect::<String>();
        // print!("{}", indent);
        // println!("{} {} {}", alpha, beta, depth);
        // print_board(black, white, (5 - depth) * 2);
    }

    let (valid_mask, valid) = get_valid_pos(black, white);

    if depth == 0 {
        let score = black.count_ones() as i32 - white.count_ones() as i32;
        // print!("{} ", score);
        // println!("b {} w {}", black.count_ones(), white.count_ones());
        score
    } else if valid_mask == 0 {
        -simple_search(white, black, depth - 1)
    } else {
        let mut best = -INF;
        for i in 0..64 {
            let mov = 1 << i;
            if valid_mask & mov == mov {
                let rev = get_reverse_mask(black, white, &valid, mov).expect("err");
                let black = black | mov | rev;
                let white = white ^ rev;
                let cand = simple_search(white, black, depth - 1);
                best = max(best, -cand);
            }
        }
        // print!("{} ", alpha);
        best
    }
}

fn eval_move(black: Mask, white: Mask, i: usize, j: usize, valid: &[(Mask, Mask); 4]) -> i32 {
    let mov = put(0, i, j);
    let reverse = get_reverse_mask(black, white, valid, mov);
    let rev = reverse.expect("err!");
    let next_black = black | rev | mov;
    let next_white = white ^ rev;
    println!("put at ({}, {})", i, j);

    let depth = 8;

    let score = -alpha_beta(next_white, next_black, -INF, INF, false, depth);
    println!("score = {}", score);

    let score = -simple_search(next_white, next_black, depth);
    println!("score = {}", score);

    print_board(next_black, next_white);
    score
}

fn ai_player(
    black: Mask,
    white: Mask,
    valid_mask: Mask,
    valid: &[(Mask, Mask); 4],
) -> Option<(usize, usize)> {
    unimplemented!()
}

fn random_player(valid_mask: Mask, rnd: &mut Xor128) -> Option<(usize, usize)> {
    let mut cand = vec![];
    for i in 0..H {
        for j in 0..8 {
            let mov = put(0, i, j);
            if valid_mask & mov == mov {
                cand.push((i, j));
            }
        }
    }
    rnd.sample(&cand).cloned()
}

fn read_line() -> String {
    let mut line = String::new();
    stdin()
        .read_line(&mut line)
        .expect("failed reading from stdin");
    line.trim().to_string()
}

fn human_player(black: Mask, white: Mask, valid_mask: Mask) -> Option<(usize, usize)> {
    let mut markers: Vec<_> = "123456789abcdefghijklmnpqrstuvwxyzABCDEFGHIJKLMNPQRSTUVWXYZ-!?()"
        .chars()
        .rev()
        .collect();
    debug_assert_eq!(markers.len(), 64);
    let mut map = BTreeMap::new();
    let mut cand = Vec::new();

    println!("Current board is: ");
    {
        let mut g = vec![vec![' '; H * 2 + 1]; W * 2 + 1];
        write_board(&mut g, black, white);
        for i in 0..H {
            for j in 0..H {
                if get(valid_mask, i, j) {
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

    if valid_mask == 0 {
        None
    } else {
        let mut c = 'O';
        while map.get(&c).is_none() {
            print!(
                "Type any character of [{}]: ",
                cand.iter().collect::<String>()
            );
            stdout().flush().unwrap();
            let t = read_line();
            if t.len() == 1 {
                c = t.chars().next().unwrap();
            }
        }
        map.get(&c).cloned()
    }
}

fn is_finished(black: Mask, white: Mask) -> bool {
    let (a, _) = get_valid_pos(black, white);
    let (b, _) = get_valid_pos(white, black);
    a == 0 && b == 0
}

fn main() {
    println!("Do you want to move at first or second?");
    print!("Type f (default) or s: ");
    stdout().flush().unwrap();
    let is_first = {
        let res = read_line();
        res == "f" || res == "F"
    };

    let (mut black, mut white) = initial_state();
    let mut rnd = Xor128::from_seed(28);

    let mut history = vec![];

    for turn in 0..usize::max_value() {
        if is_finished(black, white) {
            println!("Finish!");
            break;
        }

        let (valid_mask, valid) = get_valid_pos(black, white);

        let mov = if is_first ^ (turn % 2 == 1) {
            println!("Your turn.");
            // human_player(black, white, valid_mask)
            random_player(valid_mask, &mut rnd)
        } else {
            println!("AI's turn.");
            random_player(valid_mask, &mut rnd)
        };

        let (next_black, next_white) = if let Some((r, c)) = mov {
            println!("Move ({} {}) was choosen.", r, c);
            let mov = put(0, r, c);
            let rev = get_reverse_mask(black, white, &valid, mov).expect("invalid move");
            (white ^ rev, black | mov | rev)
        } else {
            println!("Passed.");
            (white, black)
        };

        history.push(mov);

        black = next_black;
        white = next_white;
        print_board(black, white);
    }

    println!("Final result:");
    print_board(black, white);
    let b = black.count_ones();
    let w = white.count_ones();
    println!("You : {}, AI : {}", b, w);
    if b > w {
        println!("You win!");
    } else {
        println!("You Lose!");
    }
}
