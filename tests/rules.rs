//! Chess rules: castling, en passant, promotion, check, mate, draws, plus a
//! random-playout fuzz that checks invariants and runs the evaluator for panics.

use chess::board::{parse_square, Board, Color, PieceKind};
use chess::evaluate::{evaluate_position, list3_all};
use chess::position::RecordedMove;

fn b(fen: &str) -> Board {
    Board::from_fen(fen).unwrap()
}
fn sq(s: &str) -> u8 {
    parse_square(s).unwrap()
}
fn can(board: &Board, from: &str, to: &str) -> bool {
    board.legal_moves().iter().any(|m| m.from == sq(from) && m.to == sq(to))
}

#[test]
fn castling_rules() {
    let open = b("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1");
    assert!(can(&open, "e1", "g1") && can(&open, "e1", "c1"));
    // Cannot castle out of, through, or into check; b1 may be attacked for queenside.
    assert!(!can(&b("r3k2r/8/8/8/8/8/4r3/R3K2R w KQkq - 0 1"), "e1", "g1"));
    assert!(!can(&b("r3k2r/8/8/8/8/8/5r2/R3K2R w KQkq - 0 1"), "e1", "g1"));
    assert!(!can(&b("r3k2r/8/8/8/8/8/6r1/R3K2R w KQkq - 0 1"), "e1", "g1"));
    assert!(can(&b("r3k2r/8/8/8/8/8/1r6/R3K2R w KQkq - 0 1"), "e1", "c1"));
    assert!(!can(&b("r3k2r/8/8/8/8/8/3r4/R3K2R w KQkq - 0 1"), "e1", "c1"));
    // Pieces in the way.
    assert!(!can(&b("r3k2r/8/8/8/8/8/8/RN2K2R w KQkq - 0 1"), "e1", "c1"));
    // Rights are lost when the king or a rook moves, or a rook is captured.
    let mut g = open;
    g.make(&g.find_move(sq("h1"), sq("h2"), None).unwrap());
    g.make(&g.find_move(sq("a8"), sq("a7"), None).unwrap());
    assert!(!can(&g, "e1", "g1") && can(&g, "e1", "c1"));
    let cap = b("r3k2r/8/8/8/8/8/6b1/R3K2R b KQkq - 0 1");
    let mut c = cap;
    c.make(&c.find_move(sq("g2"), sq("h1"), None).unwrap());
    assert!(!c.castling_rights(Color::White).king_side);
    // Castling moves the rook.
    let mut k = open;
    k.make(&k.find_move(sq("e1"), sq("g1"), None).unwrap());
    assert_eq!(k.piece_at(sq("f1")).unwrap().kind, PieceKind::Rook);
    assert!(k.piece_at(sq("h1")).is_none());
}

#[test]
fn en_passant_rules() {
    let mut g = b("4k3/3p4/8/4P3/8/8/8/4K3 b - - 0 1");
    g.make(&g.find_move(sq("d7"), sq("d5"), None).unwrap());
    assert!(can(&g, "e5", "d6"));
    let m = g.find_move(sq("e5"), sq("d6"), None).unwrap();
    assert!(m.is_capture());
    g.make(&m);
    assert!(g.piece_at(sq("d5")).is_none());
    // The right expires after one move.
    let mut h = b("4k3/3p4/8/4P3/8/8/7P/4K3 b - - 0 1");
    h.make(&h.find_move(sq("d7"), sq("d5"), None).unwrap());
    h.make(&h.find_move(sq("h2"), sq("h3"), None).unwrap());
    h.make(&h.find_move(sq("e8"), sq("e7"), None).unwrap());
    assert!(!can(&h, "e5", "d6"));
    // A bogus en passant square in a FEN is ignored.
    assert!(Board::from_fen("4k3/8/8/8/8/8/3P4/4K3 w - e3 0 1").is_err());
    // En passant that exposes the king along the rank is illegal.
    let pinned = b("8/8/8/KPp4r/8/8/8/4k3 w - c6 0 1");
    assert!(!can(&pinned, "b5", "c6"));
}

#[test]
fn promotion_rules() {
    let p = b("4k3/P7/8/8/8/8/8/4K3 w - - 0 1");
    let promos: Vec<_> = p.legal_moves().into_iter().filter(|m| m.from == sq("a7")).collect();
    assert_eq!(promos.len(), 4);
    assert!(promos.iter().all(|m| m.promotion.is_some()));
}

#[test]
fn check_mate_stalemate_and_draws() {
    let mut g = Board::start();
    for san in ["f3", "e5", "g4", "Qh4#"] {
        let m = g.parse_san(san).unwrap();
        g.make(&m);
    }
    assert!(g.is_checkmate() && !g.is_stalemate());
    assert!(b("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1").is_stalemate());
    assert!(b("8/8/4k3/8/8/4K3/8/8 w - - 0 1").is_insufficient_material());
    assert!(b("8/8/4k3/8/8/4KB2/8/8 w - - 0 1").is_insufficient_material());
    assert!(!b("8/8/4k3/8/8/4KR2/8/8 w - - 0 1").is_insufficient_material());
    assert!(!b("8/8/4k3/8/8/4KP2/8/8 w - - 0 1").is_insufficient_material());
    assert!(b("8/8/4k3/8/8/4K3/8/8 w - - 100 80").is_fifty_move_draw());
    // A pinned piece may not move off the line; a king may not step into check.
    let pin = b("4r1k1/8/8/8/8/8/4N3/4K3 w - - 0 1");
    assert!(pin.legal_moves().iter().all(|m| m.from != sq("e2")));
    // The king may not step onto an attacked square (the e-file rook covers e2).
    assert!(!can(&pin, "e1", "e2"));
}

#[test]
fn fen_validation_rejects_impossible_positions() {
    assert!(Board::from_fen("4k3/8/8/8/8/8/7P/4K3 w - - 0 1").is_ok());
    assert!(Board::from_fen("P3k3/8/8/8/8/8/8/4K3 w - - 0 1").is_err(), "pawn on the back rank");
    assert!(Board::from_fen("8/8/8/3kK3/8/8/8/8 w - - 0 1").is_err(), "adjacent kings");
    assert!(Board::from_fen("4k3/8/8/8/8/8/8/4K3 w - e5 0 1").is_err(), "bad en passant rank");
}

/// Tiny deterministic generator so the test needs no dependencies.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

#[test]
fn random_games_keep_every_invariant() {
    let mut rng = Rng(0x9E3779B97F4A7C15);
    for game in 0..20 {
        let mut board = Board::start();
        let mut moves: Vec<RecordedMove> = Vec::new();
        for ply in 0..160 {
            let legal = board.legal_moves();
            if legal.is_empty() || board.is_insufficient_material() {
                break;
            }
            // FEN round trip, check consistency.
            let again = Board::from_fen(&board.fen()).unwrap();
            assert_eq!(again.legal_moves().len(), legal.len(), "game {game} ply {ply} {}", board.fen());
            assert!(!board.in_check(board.turn().other()), "side not to move in check: {}", board.fen());
            for m in &legal {
                // SAN round trip.
                let san = board.san_with(m, &legal);
                assert_eq!(board.parse_san(&san), Some(*m), "{san} in {}", board.fen());
            }
            let m = legal[(rng.next() % legal.len() as u64) as usize];
            moves.push(RecordedMove::new(&board, &m));
            board.make(&m);
            if ply % 7 == 0 {
                let side = board.turn();
                let _ = evaluate_position(&board, side, &moves);
                let _ = evaluate_position(&board, side.other(), &moves);
                if ply % 21 == 0 {
                    let _ = list3_all(&board, side, &moves);
                }
            }
        }
    }
}
