mod reversi;

use reversi::*;
use std::collections::VecDeque;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
    let num_games = 100;
    let num_threads = 14;

    let seeds = Arc::new(Mutex::new((0..num_games).collect::<VecDeque<u32>>()));
    let (tx, rx) = channel();
    for _ in 0..num_threads {
        let seeds = seeds.clone();
        let tx = tx.clone();
        thread::spawn(move || loop {
            let seed = {
                let mut seeds = seeds.lock().unwrap();
                seeds.pop_front()
            };
            if let Some(seed) = seed {
                let random = Box::new(RandomPlayer::new(seed));
                let alpha_beta = Box::new(AlphaBetaSearchPlayer::new(seed));
                let (black, white): (Box<dyn Player>, Box<dyn Player>) = if seed < num_games / 2 {
                    (alpha_beta, random)
                } else {
                    (random, alpha_beta)
                };
                let names = (black.name(), white.name());
                let mut gm = GameManager::new(black, white);
                gm.verbose = false;
                gm.playout();
                let result = gm.result.unwrap();
                tx.send((names, result)).unwrap();
            } else {
                break;
            }
        });
    }

    let mut random_win = 0;
    let mut alphabeta_win = 0;
    for _ in 0..num_games {
        let ((black_name, white_name), result) = rx.recv().unwrap();
        let (black_count, white_count) = result.disks;
        let (black_mark, white_mark) = if black_count > white_count {
            ("O", " ")
        } else if black_count < white_count {
            (" ", "O")
        } else {
            (" ", " ")
        };
        println!(
            "\t{:1} {:>12} {:>2} x {:<2} {:<12} {:1}",
            black_mark, black_name, black_count, white_count, white_name, white_mark
        );
        if (black_name == "Random") ^ (black_count > white_count) {
            alphabeta_win += 1;
        } else {
            random_win += 1;
        }
    }

    println!("{} {}", alphabeta_win, random_win);
}
