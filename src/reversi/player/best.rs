//! The engine's current strongest AI, exposed behind a stable name so callers
//! (notably the WebAssembly API in `src/wasm.rs`) don't depend on which concrete
//! search engine is currently "best". To promote a new engine, change only the
//! wrapped type / delegation in this file.
use crate::reversi::bitboard;
use crate::reversi::player::alphabeta5::AlphaBeta5Player;
use crate::reversi::player::Player;

/// The current best AI. Delegates to whichever concrete engine is strongest.
pub struct BestAiPlayer(AlphaBeta5Player);

impl BestAiPlayer {
    pub fn new(seed: u32) -> Self {
        BestAiPlayer(AlphaBeta5Player::new(seed))
    }
}

/// Constructs the current best AI. Single point of change when a stronger
/// engine is added.
pub fn get_best_ai_player(seed: u32) -> BestAiPlayer {
    BestAiPlayer::new(seed)
}

impl Player for BestAiPlayer {
    fn next(&mut self, board: &bitboard::Board) -> Option<bitboard::Mask> {
        self.0.next(board)
    }
    fn name(&self) -> &'static str {
        self.0.name()
    }
}
