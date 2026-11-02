//! Lists 1-4 read the stored Step 6 search. They must agree with the small
//! in-memory reader that scores the same positions without a stored search,
//! and the stored evidence must survive a round trip through the future file.

use std::path::PathBuf;

use chess::board::{Board, Color, START_FEN};
use chess::evaluate::{
    evaluate_position, evaluate_stored, list3_all, list3_all_stored, list4_after, list4_after_stored, Evaluation,
};
use chess::files::GameFiles;
use chess::position::play_sans;
use chess::search::{full_search, FutureStore, Progress, SEARCH_PLIES};

fn summary(e: &Evaluation) -> (Vec<(&'static str, u32)>, Vec<(String, i64)>) {
    (
        e.hits.iter().map(|h| (h.name, h.score)).collect(),
        e.captures.iter().map(|c| (format!("{}/{}", c.strategy_name, c.san), c.net)).collect(),
    )
}

fn check_position(sans: &[&str]) {
    let (board, moves) = play_sans(sans).unwrap();
    let report = full_search(&board, &moves, &Progress::default()).unwrap();
    assert_eq!(report.fen, board.fen());
    assert_eq!(report.counts.len(), SEARCH_PLIES);

    // The store must survive being written to and read back from the future file.
    let mut text = format!("Position: {}\nStatus: complete in 0 s\n", report.fen);
    for l in report.store.to_lines() {
        text.push_str(&l);
        text.push('\n');
    }
    let store = FutureStore::from_text(&text).expect("future file parses");
    assert_eq!(store.entries, report.store.entries);

    // List 1 and List 2, for both players, whoever is to move.
    for side in Color::BOTH {
        let stored = evaluate_stored(&board, side, &moves, &store, &[]);
        let direct = evaluate_position(&board, side, &moves);
        assert_eq!(summary(&stored), summary(&direct), "List 1/2 for {side:?} after {sans:?}");
        // A strategy that makes progress within one full turn is first seen by move 2.
        for h in &stored.hits {
            if h.progress_within_turn {
                assert!(h.progress_in.is_some_and(|n| n <= 2), "{} horizon", h.name);
            }
            if h.completion_within_turn {
                assert!(h.completion_in.is_some_and(|n| n <= 2), "{} horizon", h.name);
            }
        }
    }

    // List 3 for every legal move, and List 4 for every reply, for the side to move.
    let side = board.turn();
    let stored3 = list3_all_stored(&board, side, &moves, &store);
    let direct3 = list3_all(&board, side, &moves);
    assert_eq!(stored3.len(), direct3.len());
    for (a, b) in stored3.iter().zip(direct3.iter()) {
        assert_eq!(a.san, b.san);
        assert_eq!(summary(&a.evaluation), summary(&b.evaluation), "List 3 after {}", a.san);
    }
    for (a, b) in stored3.iter().zip(direct3.iter()).take(6) {
        let legal = a.board.legal_moves();
        for r in &legal {
            let reply = a.board.san_with(r, &legal);
            let (_, e1) = list4_after_stored(a, &reply, &store).unwrap();
            let (_, e2) = list4_after(b, &reply).unwrap();
            assert_eq!(summary(&e1), summary(&e2), "List 4 after {} {}", a.san, reply);
        }
    }
}

#[test]
fn start_position_lists_match_the_in_memory_reader() {
    check_position(&[]);
}

#[test]
fn middlegame_lists_match_the_in_memory_reader() {
    check_position(&["e4", "e5", "Nf3", "Nc6", "Bc4", "Nf6"]);
}

#[test]
fn black_to_move_lists_match_the_in_memory_reader() {
    check_position(&["d4", "d5", "c4", "e6", "Nc3"]);
}

#[test]
fn the_search_counts_the_standard_start_numbers_to_six_moves() {
    let r = full_search(&Board::from_fen(START_FEN).unwrap(), &[], &Progress::default()).unwrap();
    assert_eq!(r.counts, [20, 400, 8_902, 197_281, 4_865_609, 119_060_324]);
    assert_eq!(r.total_nodes(), 124_132_536);
    // The searched position, 20 first moves, and 400 move/reply pairs are stored.
    assert_eq!(r.store.entries.len(), 1 + 20 + 400);
}

/// The past and the future are read back from the files, not from memory.
#[test]
fn lists_read_the_past_and_the_future_from_the_game_files() {
    let dir: PathBuf = std::env::temp_dir().join(format!("chess_files_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut files = GameFiles::new(&dir).unwrap();
    let mut board = Board::start();
    files.start_game(&board, &[]).unwrap();
    for san in ["e4", "e5", "Nf3"] {
        let before = board;
        let m = board.parse_san(san).unwrap();
        board.make(&m);
        files.record_turn(&before, &m, san, &board).unwrap();
    }
    let past = files.history_moves();
    assert_eq!(past.len(), 3);
    assert_eq!(past.iter().filter(|m| m.color == Color::White).count(), 2);

    let store = files.stored_search(&board).expect("the finished search is in the future file");
    for side in Color::BOTH {
        let from_files = evaluate_stored(&board, side, &past, &store, &[]);
        let direct = evaluate_position(&board, side, &past);
        assert_eq!(summary(&from_files), summary(&direct));
    }
    // A new game clears all four files.
    files.start_game(&Board::start(), &[]).unwrap();
    assert!(files.history_moves().is_empty());
    drop(files);
    let _ = std::fs::remove_dir_all(&dir);
}
