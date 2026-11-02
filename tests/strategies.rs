//! Port of `tests/strategies.test.ts`: development scores, all 20 strategies,
//! and strategy captures.

use chess::board::{parse_square, Board, Color};
use chess::evaluate::{evaluate_fen, evaluate_position, list3_all, list3_after, list4_after, Stage, StrategyHit};
use chess::position::play_sans;
use chess::presets::{preset_by_id, presets};

const START: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

fn hit<'a>(hits: &'a [StrategyHit], id: &str) -> Option<&'a StrategyHit> {
    hits.iter().find(|h| h.id == id)
}

fn expect_stage(hits: &[StrategyHit], id: &str, stage: Stage, score: u32) {
    let found = hit(hits, id).unwrap_or_else(|| panic!("{id} missing"));
    assert_eq!((found.stage, found.score), (stage, score), "{id}");
}

fn stage_name(s: Stage) -> &'static str {
    match s {
        Stage::Full => "full",
        Stage::Partial => "partial",
        Stage::Beginning => "beginning",
    }
}

// ----- development scoring --------------------------------------------------

#[test]
fn marks_the_starting_position_as_two_beginning_strategies_at_45() {
    let e = evaluate_fen(START, Color::White, &[]).unwrap();
    let mut got: Vec<String> = e.hits.iter().map(|h| format!("{}:{}:{}", h.id, h.score, stage_name(h.stage))).collect();
    got.sort();
    assert_eq!(got, vec!["central-pawn-occupation:45:beginning", "king-shelter:45:beginning"]);
    assert!(e.hits.iter().all(|h| h.progress_within_turn && !h.completion));
}

#[test]
fn raises_central_occupation_to_partial_once_the_e_pawn_arrives() {
    let (board, moves) = play_sans(&["e4"]).unwrap();
    let e = evaluate_position(&board, Color::White, &moves);
    expect_stage(&e.hits, "central-pawn-occupation", Stage::Partial, 60);
    assert!(!e.side_to_move);
    assert!(e.captures.is_empty());
}

#[test]
fn scores_a_completable_center_at_75_rather_than_fully_developed() {
    let e = evaluate_fen("4k3/8/8/8/4P3/2P2P2/3P4/4K3 w - - 0 1", Color::White, &[]).unwrap();
    expect_stage(&e.hits, "central-pawn-occupation", Stage::Partial, 75);
    let h = hit(&e.hits, "central-pawn-occupation").unwrap();
    assert!(h.completion_within_turn);
    assert!(!h.completion);
}

#[test]
fn does_not_list_a_strategy_from_a_future_line_alone() {
    let e = evaluate_fen(START, Color::White, &[]).unwrap();
    assert!(hit(&e.hits, "open-file-control").is_none());
    assert!(hit(&e.hits, "kingside-pawn-storm").is_none());
}

// ----- the twenty strategies --------------------------------------------------

#[test]
fn recognizes_every_strategy_as_fully_developed_in_its_reference_position() {
    let cases = [
        ("supported-center", "central-pawn-occupation"),
        ("central-break", "central-pawn-break"),
        ("storm", "kingside-pawn-storm"),
        ("minority", "queenside-minority-attack"),
        ("king-shelter", "king-shelter"),
        ("opposite", "opposite-side-castling-attack"),
        ("fianchetto", "fianchetto"),
        ("battery", "open-file-control"),
        ("seventh", "seventh-rank-invasion"),
        ("passed-pawn", "passed-pawn-advance"),
        ("isolated", "isolated-pawn-attack"),
        ("backward", "backward-pawn-pressure"),
        ("outpost", "knight-outpost"),
        ("bishop-pair", "bishop-pair"),
        ("pin", "pin"),
        ("discovery", "discovered-attack"),
        ("fork", "fork"),
        ("skewer", "skewer"),
        ("battery", "major-piece-battery"),
        ("promotion", "promotion"),
    ];
    let mut failures = Vec::new();
    for (preset_id, id) in cases {
        let p = preset_by_id(preset_id).unwrap();
        let e = evaluate_position(&p.board(), p.side, &p.moves);
        match hit(&e.hits, id) {
            Some(h) if h.stage == Stage::Full && h.score == 100 => {}
            other => {
                let summary: Vec<String> = e.hits.iter().map(|h| format!("{}:{}:{}", h.id, stage_name(h.stage), h.score)).collect();
                failures.push(format!(
                    "{id} via {preset_id} -> {}; saw {}",
                    other.map_or("missing".to_string(), |h| format!("{}:{} ({})", stage_name(h.stage), h.score, h.evidence)),
                    summary.join(", ")
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn sorts_fully_developed_strategies_above_partial_and_beginning_ones() {
    let p = preset_by_id("supported-center").unwrap();
    let e = evaluate_position(&p.board(), Color::White, &[]);
    let stages: Vec<Stage> = e.hits.iter().map(|h| h.stage).collect();
    let mut sorted = stages.clone();
    sorted.sort();
    assert_eq!(stages, sorted);
    assert_eq!(stages[0], Stage::Full);
}

#[test]
fn keeps_a_blocked_promotion_partial_when_the_pawn_cannot_legally_promote() {
    let e = evaluate_fen("4k3/4P3/8/8/8/8/8/4K3 w - - 0 1", Color::White, &[]).unwrap();
    let h = hit(&e.hits, "promotion").unwrap();
    assert!(!h.completion);
    assert_ne!(h.stage, Stage::Full);
}

#[test]
fn still_counts_a_pinned_bishop_toward_a_pin_when_the_bishop_itself_has_the_line() {
    let e = evaluate_fen("4k3/8/2n5/1B6/8/8/8/4K3 w - - 0 1", Color::White, &[]).unwrap();
    let h = hit(&e.hits, "pin").unwrap();
    assert!(h.participants.iter().any(|u| Some(u.square) == parse_square("b5")));
    assert!(h.targets.iter().any(|u| Some(u.square) == parse_square("c6")));
}

// ----- strategy captures ------------------------------------------------------

#[test]
fn links_the_bishops_capture_of_the_pinned_knight_and_prices_the_recapture() {
    let e = evaluate_fen("4k3/8/2n5/1B6/8/8/8/4K3 w - - 0 1", Color::White, &[]).unwrap();
    let c = e.captures.iter().find(|r| r.san.starts_with("Bxc6") && r.strategy_id == "pin").expect("Bxc6 for pin");
    assert_eq!((c.expected_gain, c.potential_loss, c.net), (320, 0, 320));
}

#[test]
fn sorts_strategy_captures_by_net_value() {
    let p = preset_by_id("fork").unwrap();
    let e = evaluate_position(&p.board(), Color::White, &[]);
    let nets: Vec<i64> = e.captures.iter().map(|r| r.net).collect();
    let mut sorted = nets.clone();
    sorted.sort_by(|a, b| b.cmp(a));
    assert_eq!(nets, sorted);
    assert!(e.captures.iter().any(|r| r.strategy_id == "fork" && r.victim == chess::PieceKind::Queen));
}

#[test]
fn prices_every_legal_capture_including_ones_no_strategy_claims() {
    let e = evaluate_fen("4k3/8/8/3p4/8/8/8/3QK3 w - - 0 1", Color::White, &[]).unwrap();
    let c = e.legal_captures.iter().find(|r| r.san == "Qxd5").unwrap();
    assert_eq!(c.valuation.net, 100);
}

#[test]
fn loads_every_preset_without_panicking() {
    for p in presets() {
        let e = evaluate_position(&p.board(), p.side, &p.moves);
        assert!(!e.hits.is_empty(), "{}", p.id);
    }
}

// ----- additions for the Rust port --------------------------------------------

#[test]
fn list3_covers_every_legal_next_move() {
    let board = Board::start();
    let branches = list3_all(&board, Color::White, &[]);
    assert_eq!(branches.len(), 20);
    let e4 = branches.iter().find(|b| b.san == "e4").unwrap();
    expect_stage(&e4.evaluation.hits, "central-pawn-occupation", Stage::Partial, 60);
}

#[test]
fn list4_prices_captures_after_the_reply() {
    let (board, moves) = play_sans(&["e4", "d5"]).unwrap();
    let branch = list3_after(&board, Color::White, &moves, "Nc3").unwrap();
    let (_, e) = list4_after(&branch, "dxe4").unwrap();
    assert!(e.side_to_move);
    assert!(e.legal_captures.iter().any(|c| c.san == "Nxe4"));
}

#[test]
fn evaluating_while_the_opponent_is_in_check_does_not_crash() {
    // White has just given check; Black is to move. The TypeScript version threw here.
    let (board, moves) = play_sans(&["e4", "f5", "Qh5+"]).unwrap();
    let e = evaluate_position(&board, Color::White, &moves);
    assert!(!e.side_to_move);
}
