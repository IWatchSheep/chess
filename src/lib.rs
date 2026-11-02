//! Forming Strategies chess program (Rust port).
//!
//! * `board`      – board, legal moves, FEN and SAN (replaces chess.js)
//! * `geometry`   – piece values and geometric attack helpers
//! * `position`   – recorded moves, history window, evaluation context
//! * `exchange`   – capture valuation (Step 11 / 14.3)
//! * `strategies` – the 20 strategies (Step 14.4)
//! * `evaluate`   – development scores and Lists 1–4 (Steps 8–13)
//! * `presets`    – reference positions, one per strategy
//! * `search`     – the three-full-turn (six-ply) future search (Step 6)
//! * `files`      – history and future-search files (Steps 4, 5, 7)

pub mod board;
pub mod evaluate;
pub mod exchange;
pub mod files;
pub mod geometry;
pub mod position;
pub mod presets;
pub mod search;
pub mod strategies;

pub use board::{Board, Color, Move, Piece, PieceKind, Square};
