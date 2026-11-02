//! Port of `tests/exchange.test.ts`: the capture valuation examples of Step 14.3.

use chess::board::Board;
use chess::exchange::{value_capture, CaptureValuation};
use chess::geometry::MATE_BONUS;

fn price(fen: &str, san: &str) -> CaptureValuation {
    let board = Board::from_fen(fen).unwrap();
    let legal = board.legal_moves();
    let m = legal
        .iter()
        .find(|m| board.san_with(m, &legal) == san)
        .unwrap_or_else(|| panic!("missing {san} in {fen}\n{board}"));
    value_capture(&board, m)
}

#[test]
fn prices_an_undefended_pawn_as_pure_gain() {
    let r = price("4k3/8/8/3p4/8/8/8/3QK3 w - - 0 1", "Qxd5");
    assert_eq!((r.expected_gain, r.potential_loss, r.net, r.mate), (100, 0, 100, false));
    assert!(r.recapture_sans.is_empty());
}

#[test]
fn charges_the_queen_when_a_knight_recaptures_it() {
    let r = price("4k3/4n3/8/3p4/8/8/8/3QK3 w - - 0 1", "Qxd5");
    assert_eq!((r.expected_gain, r.potential_loss, r.net), (100, 900, -800));
    assert_eq!(r.recapture_sans, vec!["Nxd5"]);
}

#[test]
fn stops_the_opponent_from_recapturing_into_a_lost_rook() {
    let r = price("4r2k/8/8/4q3/3P4/8/8/4R1K1 w - - 0 1", "dxe5");
    assert_eq!((r.expected_gain, r.potential_loss, r.net), (900, 0, 900));
    assert!(r.recapture_sans.is_empty());
}

#[test]
fn continues_when_the_knight_sacrifice_wins_the_queen_and_the_pawn() {
    let r = price("4k3/4p3/5q2/6P1/6N1/8/8/4K3 w - - 0 1", "Nxf6+");
    assert_eq!((r.expected_gain, r.potential_loss, r.net), (1000, 320, 680));
    assert_eq!(r.recapture_sans, vec!["exf6", "gxf6"]);
}

#[test]
fn recaptures_with_the_pawn_rather_than_the_queen() {
    let r = price("3q3k/8/2p5/3r4/8/8/8/3QK3 w - - 0 1", "Qxd5");
    assert_eq!((r.expected_gain, r.potential_loss, r.net), (500, 900, -400));
    assert_eq!(r.recapture_sans[0], "cxd5");
}

#[test]
fn prices_en_passant_and_the_rook_recapture() {
    let r = price("3rk3/8/8/3pP3/8/8/8/4K3 w - d6 0 1", "exd6");
    assert_eq!((r.expected_gain, r.potential_loss, r.net), (100, 100, 0));
    assert_eq!(r.recapture_sans, vec!["Rxd6"]);
}

#[test]
fn adds_the_mate_bonus_and_no_loss_when_the_capture_checkmates() {
    let r = price("k7/1p6/1Q6/2N5/8/8/8/4K3 w - - 0 1", "Qxb7#");
    assert!(r.mate);
    assert_eq!(r.potential_loss, 0);
    assert_eq!(r.expected_gain, 100 + MATE_BONUS);
    assert_eq!(r.net, 100 + MATE_BONUS);
}

#[test]
fn adds_promotion_value_before_a_recapture_of_the_new_queen() {
    let r = price("7r/5P1k/8/8/8/8/8/4K3 w - - 0 1", "f8=Q");
    assert_eq!((r.expected_gain, r.potential_loss, r.net), (800, 900, -100));
    assert_eq!(r.recapture_sans, vec!["Rxf8"]);
}
