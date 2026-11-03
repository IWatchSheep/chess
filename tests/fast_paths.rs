//! The faster ways of reading the strategies give exactly the original answers.
//!
//! * `attacks_square` (worked out directly) equals the original definition,
//!   `attacked_squares(..).contains(..)`, for every piece and square.
//! * Every strategy read with `EvalContext::flags_only` (as the six-move search
//!   reads it) has the same foundation, progress and completion as the full
//!   reading the lists show.

use chess::board::{Board, Color, START_FEN};
use chess::geometry::{attacks_square, attacks_square_reference, Position};
use chess::position::EvalContext;
use chess::presets::presets;
use chess::strategies::STRATEGIES;

/// Positions two plies from a spread of openings, middlegames, endgames and every preset.
fn positions() -> Vec<Board> {
    let mut roots: Vec<Board> = [
        START_FEN,
        "r1bqk2r/pppp1ppp/2n2n2/2b1p3/2B1P3/2N2N2/PPPP1PPP/R1BQK2R w KQkq - 4 5",
        "r3k2r/1b2qppp/p1n1pn2/1pb5/4P3/1BN2N2/PPP1QPPP/R1B2RK1 w kq - 0 1",
        "8/2k5/3p4/p2P1p2/P2P1P2/8/3K4/8 w - - 0 1",
        "r3k3/1P6/8/8/8/8/6p1/4K2R w Kq - 0 1",
        "2kr3r/ppp2ppp/2n5/3q4/3P4/2P2N2/P4PPP/R2Q1RK1 b - - 0 1",
    ]
    .iter()
    .map(|f| Board::from_fen(f).unwrap())
    .collect();
    roots.extend(presets().iter().map(|p| p.board()));
    let mut out = Vec::new();
    for b in roots {
        out.push(b);
        for m in b.legal_moves() {
            let c = b.play(&m);
            out.push(c);
            for m2 in c.legal_moves().into_iter().step_by(3) {
                out.push(c.play(&m2));
            }
        }
    }
    out
}

#[test]
fn attacks_square_matches_its_definition() {
    for b in positions() {
        let pos = Position::read(&b);
        for u in &pos.units {
            for t in 0..64u8 {
                assert_eq!(attacks_square(&pos, u, t), attacks_square_reference(&pos, u, t), "{} {:?} -> {}", b.fen(), u, t);
            }
        }
    }
}

#[test]
fn flags_only_readings_match_full_readings() {
    let moves = vec![];
    for b in positions() {
        for side in Color::BOTH {
            let full = EvalContext::new(&b, side, &moves);
            let quick = EvalContext::flags_only(&b, side, &moves);
            for d in STRATEGIES.iter() {
                let a = (d.read)(&full);
                let q = (d.read)(&quick);
                assert_eq!((a.foundation, a.progress, a.completion), (q.foundation, q.progress, q.completion), "{} for {} on {}", d.name, side.name(), b.fen());
            }
        }
    }
}
