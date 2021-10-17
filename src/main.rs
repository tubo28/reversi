mod reversi;

use player::alphabeta::AlphaBetaSearchPlayer;
use player::cli::HumanPlayer;
use player::random::RandomPlayer;
use reversi::gm;
use reversi::player;
use reversi::util;
use std::io::{stdout, Write};

fn main() {
    println!("choose players.");
    println!("  a : AI (alpha-beta search, default)");
    println!("  b : random");
    println!("  c : human (from keyboard)");

    let labels = ["first (black)", "second (white)"];
    let mut players = Vec::new();
    for label in labels.iter() {
        print!("{} player? [A/b/c]: ", label);
        stdout().flush().unwrap();
        let player = match util::read_one_char().and_then(|a| a.to_lowercase().next()) {
            Some('b') => Box::new(RandomPlayer::new(28)) as Box<dyn player::Player>,
            Some('c') => Box::new(HumanPlayer::new()) as Box<dyn player::Player>,
            _ => Box::new(AlphaBetaSearchPlayer::new(28)) as Box<dyn player::Player>,
        };
        println!("selected {}", player.name());
        players.push(player);
    }

    let black = players.swap_remove(0);
    let white = players.swap_remove(0);
    gm::GameManager::new(black, white).playout();
}
