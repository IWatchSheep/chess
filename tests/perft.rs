//! Move-generator correctness: legal move sequence counts ("perft").
//! The start-position numbers are the ones listed in Step 6.

use chess::board::{perft, Board, START_FEN};
use chess::search::{full_search, Progress};

fn count(fen: &str, depth: u32) -> u64 {
    perft(&Board::from_fen(fen).unwrap(), depth)
}

#[test]
fn start_position_matches_step_6_to_five_moves() {
    let expected = [20, 400, 8_902, 197_281, 4_865_609];
    for (d, &n) in expected.iter().enumerate() {
        assert_eq!(count(START_FEN, d as u32 + 1), n, "depth {}", d + 1);
    }
}

/// Castling, en passant, promotions and pins (standard "Kiwipete" and friends).
#[test]
fn tricky_positions() {
    assert_eq!(count("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1", 3), 97_862);
    assert_eq!(count("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1", 5), 674_624);
    assert_eq!(count("r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1", 4), 422_333);
    assert_eq!(count("rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8", 3), 62_379);
    assert_eq!(count("r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10", 3), 89_890);
}

/// The full Step 6 search from the start position. Takes a few seconds in a
/// release build; run with `cargo test --release -- --ignored`.
#[test]
#[ignore]
fn full_six_move_search_matches_step_6() {
    let r = full_search(&Board::start(), &[], &Progress::default()).unwrap();
    assert_eq!(r.counts, [20, 400, 8_902, 197_281, 4_865_609, 119_060_324]);
    assert_eq!(r.total_nodes(), 124_132_536);
}
