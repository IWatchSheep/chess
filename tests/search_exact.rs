//! The six-move search reads the strategies through all six individual moves.
//! Its stored evidence must be exactly what reading every one of the 20
//! strategies, for both players, on every position of the search gives.

use chess::board::Board;
use chess::position::RecordedMove;
use chess::search::{full_search, full_search_reference, Progress, READ_PLIES, SEARCH_PLIES};

fn same(fen: &str, moves: &[RecordedMove]) {
    let b = Board::from_fen(fen).unwrap();
    let fast = full_search(&b, moves, &Progress::default()).unwrap();
    let slow = full_search_reference(&b, moves, &Progress::default(), SEARCH_PLIES).unwrap();
    assert_eq!(fast.counts, slow.counts, "{fen}");
    assert_eq!(fast.store.entries, slow.store.entries, "{fen}");
    assert_eq!(fast.store.to_lines(), slow.store.to_lines(), "{fen}");
}

#[test]
fn strategies_are_read_through_all_six_moves() {
    assert_eq!(SEARCH_PLIES, 6);
    assert_eq!(READ_PLIES, SEARCH_PLIES);
}

#[test]
fn pawn_endings_match_the_full_reading() {
    same("4k3/8/8/3p4/8/8/4P3/4K3 w - - 0 1", &[]);
    same("8/5k2/8/2pP4/8/8/5K2/8 w - c6 0 1", &[]);
}

#[test]
fn a_position_with_castling_and_promotion_matches_the_full_reading() {
    same("r3k3/1P6/8/8/8/8/6p1/4K2R w Kq - 0 1", &[]);
}

#[test]
fn the_history_window_is_read_exactly() {
    // Central pawn break reads the turn window, which includes the game's own
    // moves before the searched position.
    let mut b = Board::from_fen("4k3/3p4/8/8/8/8/4P3/4K3 w - - 0 1").unwrap();
    let mut moves = Vec::new();
    for uci in ["e2e4", "d7d5"] {
        let m = *b.legal_moves().iter().find(|m| m.uci() == uci).unwrap();
        moves.push(RecordedMove::new(&b, &m));
        b = b.play(&m);
    }
    same(&b.fen(), &moves);
}

#[test]
fn ended_lines_match_the_full_reading() {
    // Mate and stalemate inside the six moves.
    same("6k1/5ppp/8/8/8/8/5PPP/R5K1 w - - 0 1", &[]);
    same("7k/8/6Q1/8/8/8/8/6K1 w - - 0 1", &[]);
}
