type Mask = u64;

const H: usize = 8;
const W: usize = 8;

/// 半時計回りに90度回転
#[inline]
fn rotate(x: Mask) -> Mask {
    let x = ((x << 1) & 0xAA00AA00AA00AA00) |
    ((x >> 1) & 0x0055005500550055) |
    ((x >> 8) & 0x00AA00AA00AA00AA) |
    ((x << 8) & 0x5500550055005500);

    let x = ((x << 2) & 0xCCCC0000CCCC0000) |
    ((x >> 2) & 0x0000333300003333) |
    ((x >>16) & 0x0000CCCC0000CCCC) |
    ((x <<16) & 0x3333000033330000);

    let x = ((x << 4) & 0xF0F0F0F000000000) |
    ((x >> 4) & 0x000000000F0F0F0F) |
    ((x >>32) & 0x00000000F0F0F0F0) |
    ((x <<32) & 0x0F0F0F0F00000000);
    x
}

#[inline]
fn get_valid(black: Mask, white: Mask) -> [(Mask, Mask); 4] {
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

    let mut res = [(0, 0); 4];
    let mut black = black;
    let mut white = white;
    for i in 0..4 {
        let mut left = valid_mask_from_left(black, white);
        let mut left_top = valid_mask_from_top_left(black, white);
        res[i] = (left, left_top);
        black = rotate(black);
        white = rotate(white);
    }
    res
}

fn get_rev(black: Mask, white: Mask, valid: [(Mask, Mask); 4], m: Mask)
           -> Result<Mask, ()> {
    fn transfer_left(m: Mask) -> Mask {
        ( m << 1 ) & 0xfefefefefefefefe
    }

    fn transfer_top_left(m: Mask) -> Mask {
        ( m << 9 ) & 0xfefefefefefefe00
    }

    if ((black | white) & m) != 0 {
        return Err(());
    }

    let mut res = 0;

    let mut black = black;
    let mut white = white;
    let mut m = m;

    for i in 0..4 {
        let (valid_left, valid_top_left) = valid[i];

        let rev_left = if (valid_left & m) == m {
            println!("a");
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
            println!("b");
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

fn initial_state() -> (Mask, Mask) {
        (0b_00000000_00000000_00000000_00010000_00001000_00000000_00000000_00000000,
         0b_00000000_00000000_00000000_00001000_00010000_00000000_00000000_00000000)
}

#[inline]
fn set(mask: Mask, r: usize, c: usize) -> Mask {
    mask | (1u64 << (r * 8 + c))
}

#[inline]
fn get(mask: Mask, r: usize, c: usize) -> bool {
    mask >> (r * 8 + c) & 1 == 1
}

fn write_mask(g: &mut Vec<Vec<char>>, mask: Mask, c: char) {
    for i in 0..H*2+1 {
        for j in 0..W*2+1 {
            if i % 2 == 1 && j % 2 == 1 {
                if get(mask, i / 2, j / 2) {
                    assert_eq!(g[i][j], ' ');
                    g[i][j] = c;
                }
            }
        }
    }
}

fn write_grid(g: &mut Vec<Vec<char>>) {
    for i in 0..H*2+1 {
        for j in 0..W*2+1 {
            if i % 2 != 1 || j % 2 != 1 {
                let c = if i % 2 == 0 && j % 2 == 0 {
                    '+'
                } else if i % 2 == 0 {
                    '-'
                } else {
                    '|'
                };
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

fn pos_to_mask(r: usize, c: usize) -> Mask {
    set(0, r, c)
}

fn print_board(black: Mask, white: Mask) {
    let valid = get_valid(black, white);
    let mut g = vec![vec![' '; H * 2 + 1]; W * 2 + 1];
    write_board(&mut g, black, white);
    for i in 0..4 {
        let (mut left, mut top_left) = valid[i];
        for _ in 0..4-i {
            left = rotate(left);
            top_left = rotate(top_left);
        }
        write_mask(&mut g, left, '.');
        write_mask(&mut g, top_left, '.');
    }
    for row in g.iter() {
        println!("{}", row.iter().collect::<String>());
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

fn alpha_beta(black: Mask, white: Mask) -> Mask {
    let rev = get_valid(black, white);
    let r = {
        let mut r = 0;
        for i in 0..4 {
            let (l, tl) = rev[i];
            r |= l | tl;
            r = rotate(r);
        }
        r
    };
    for i in 0..64 {
        let m = 1 << i;
        if r & m == 1 {
            let rev = get_rev(black, white, rev, m);
            assert!(rev.is_ok());
            
        }
    }

    0
}

fn main() {
    let (black, white) = initial_state();
    print_board(black, white);

    let valid = get_valid(black, white);
    for i in 0..8 {
        for j in 0..8 {
            let m = pos_to_mask(i, j);
            let rev = get_rev(black, white, valid, m);
            if let Ok(r) = rev {
                println!("{} {}", i, j);
                print_mask(r);
            }
        }
    }
}
