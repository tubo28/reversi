//! Reversi library crate.
//!
//! Exposes the game engine (`reversi` module) shared with the `main` binary,
//! plus a small `extern "C"` API (`wasm` module) for running the engine in the
//! browser via WebAssembly. See `web/` for the static frontend.
pub mod reversi;
mod wasm;
