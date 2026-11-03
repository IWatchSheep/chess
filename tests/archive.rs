//! Step 15 (rules 15.10): the archive keeps the files made by Steps 4, 5 and 6.
//!
//! Small endgame positions keep the six-move searches fast; the start-position
//! test checks the full 124,132,536-node search is what gets saved.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use chess::archive::{
    self, archive_dir, future_name, history_name, memories, read_game, rebuild_memory, recover, GameArchive, GameStatus, Outcome, WorkingFiles,
    MEMORY_INDEX, MEMORY_LIMIT,
};
use chess::board::{square_name, Board, Color, START_FEN};
use chess::evaluate::{evaluate_stored, list3_all_stored};
use chess::files::{GameFiles, HISTORY_TURNS};
use chess::search::{FutureStore, SEARCH_PLIES};

const ENDGAME: &str = "4k3/8/8/3p4/8/8/4P3/4K3 w - - 0 1";

fn temp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("chess_step15_{}_{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// One recorded move, done the way the program does it, returning the four
/// working files read independently at the moment of recording.
fn play_recorded(files: &mut GameFiles, a: &mut GameArchive, board: &mut Board, m: chess::board::Move) -> WorkingFiles {
    // 15.3: the search of the current position finishes before the move is confirmed.
    files.wait_for_searches();
    let legal = board.legal_moves();
    let san = board.san_with(&m, &legal);
    let past = files.history_moves();
    let before = *board;
    *board = board.play(&m);
    let after = *board;
    let mut seen = None;
    files
        .record_turn_then(&before, &m, &san, &after, |f| {
            let copy = WorkingFiles::read(f).unwrap();
            // An independent read of the same four files, straight from disk.
            let direct: Vec<(String, Vec<u8>)> =
                f.all_paths().iter().map(|p| (p.file_name().unwrap().to_str().unwrap().to_string(), fs::read(p).unwrap())).collect();
            assert_eq!(copy.files, direct);
            seen = Some(copy.clone());
            a.record_move(&before, &m, &san, &after, &past, &copy).unwrap();
        })
        .unwrap();
    seen.unwrap()
}

/// Record a game of `plies` moves (always the first legal move).
fn record_game(dir: &Path, fen: &str, plies: usize, outcome: Option<Outcome>) -> (PathBuf, Vec<WorkingFiles>) {
    let mut board = Board::from_fen(fen).unwrap();
    let mut files = GameFiles::new(dir).unwrap();
    files.start_game(&board, &[]).unwrap();
    let mut a = GameArchive::create(&archive_dir(dir), &board, &[]).unwrap();
    let mut seen = Vec::new();
    for _ in 0..plies {
        let m = board.legal_moves()[0];
        seen.push(play_recorded(&mut files, &mut a, &mut board, m));
    }
    match outcome {
        Some(o) => a.finish(&o).unwrap(),
        None => a.stop("the test ended").unwrap(),
    }
    (a.path().to_path_buf(), seen)
}

#[test]
fn saved_files_match_the_working_files_byte_for_byte() {
    let dir = temp_dir("bytes");
    let (path, seen) = record_game(&dir, ENDGAME, 6, None);
    let g = read_game(&path).unwrap();
    assert_eq!(g.format, archive::FORMAT);
    assert_eq!(g.moves.len(), 6);
    for (m, working) in g.moves.iter().zip(&seen) {
        assert_eq!(&m.files, working, "ply {}", m.ply);
        assert_eq!(m.files.files.len(), 4);
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn each_move_has_a_past_a_present_and_a_finished_future() {
    let dir = temp_dir("parts");
    let (path, _) = record_game(&dir, ENDGAME, 4, None);
    let g = read_game(&path).unwrap();
    for m in &g.moves {
        let side = m.side;
        // Present: the turn just played, in the history file of the side that moved,
        // with every piece of both colors before and after the move.
        let h = String::from_utf8(m.file(&history_name(side)).unwrap().to_vec()).unwrap();
        assert!(h.contains("— current turn"), "ply {}", m.ply);
        let present = &h[h.rfind("— current turn").unwrap()..];
        let before = Board::from_fen(&m.fen_before).unwrap();
        let after = Board::from_fen(&m.fen_after).unwrap();
        let (before_part, after_part) = present.split_once("-- After the move --").unwrap();
        for (part, board) in [(before_part, before), (after_part, after)] {
            for (sq, _) in board.pieces_in_board_order() {
                assert!(part.contains(&format!(" {}: ", square_name(sq))), "ply {}: {} missing", m.ply, square_name(sq));
            }
            assert!(part.contains("White pieces") && part.contains("Black pieces"));
        }
        // Past: the earlier turns, in both history files.
        if m.ply > 2 {
            assert!(h.contains("previous turn 1"));
        }
        // Future: the finished six-move search of the position where the player decided.
        let fut = String::from_utf8(m.file(&future_name(side)).unwrap().to_vec()).unwrap();
        assert!(fut.contains("Status: complete"), "ply {}", m.ply);
        assert!(fut.contains(&format!("Position: {}", m.fen_before)));
        assert_eq!(fut.lines().filter(|l| l.trim_start().starts_with("Individual move ")).count(), SEARCH_PLIES);
        assert!(m.future_store().is_some());
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_long_game_keeps_its_opening_middlegame_and_ending() {
    let dir = temp_dir("long");
    let plies = 2 * HISTORY_TURNS + 6;
    let (path, _) = record_game(&dir, ENDGAME, plies, Some(Outcome::win(Color::White, "resignation", "Black resigns. White wins.".into())));
    // The working history file keeps only the rolling window...
    let rolling = fs::read_to_string(dir.join("white_history.txt")).unwrap();
    assert!(rolling.lines().filter(|l| l.starts_with("RECORD ")).count() <= HISTORY_TURNS);
    // ...the archive keeps every move, from the first.
    let g = read_game(&path).unwrap();
    assert_eq!(g.moves.len(), plies);
    assert_eq!(g.moves[0].fen_before, ENDGAME);
    for (i, m) in g.moves.iter().enumerate() {
        assert_eq!(m.ply as usize, i + 1);
        if i > 0 {
            assert_eq!(m.fen_before, g.moves[i - 1].fen_after, "past links to present links to future");
        }
    }
    // One file, both colors.
    assert!(g.moves.iter().any(|m| m.side == Color::White) && g.moves.iter().any(|m| m.side == Color::Black));
    assert_eq!(archive::archive_files(&archive_dir(&dir)).len(), 1);
    assert_eq!(g.status, GameStatus::Finished { result_code: "1-0".into(), termination: "resignation".into(), text: "Black resigns. White wins.".into() });
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_saved_lists_are_the_official_lists() {
    let dir = temp_dir("lists");
    let mut board = Board::from_fen(ENDGAME).unwrap();
    let mut files = GameFiles::new(&dir).unwrap();
    files.start_game(&board, &[]).unwrap();
    let mut a = GameArchive::create(&archive_dir(&dir), &board, &[]).unwrap();
    let mut expected = Vec::new();
    for _ in 0..3 {
        files.wait_for_searches();
        // The official lists exactly as the game shows them, from the working files.
        let store = files.stored_search(&board).unwrap();
        let past = files.history_moves();
        let side = board.turn();
        let l1: Vec<(String, u32)> = [side, side.other()]
            .iter()
            .flat_map(|&s| evaluate_stored(&board, s, &past, &store, &[]).hits.into_iter().map(move |h| (format!("{} {}", s.name(), h.name), h.score)))
            .collect();
        let l3: Vec<(String, usize)> = list3_all_stored(&board, side, &past, &store).iter().map(|b| (b.mv.uci(), b.evaluation.hits.len())).collect();
        let m = board.legal_moves()[0];
        expected.push((l1, l3, m.uci()));
        play_recorded(&mut files, &mut a, &mut board, m);
    }
    a.stop("the test ended").unwrap();
    let g = read_game(a.path()).unwrap();
    for (m, (l1, l3, played)) in g.moves.iter().zip(expected) {
        let saved_l1: Vec<(String, u32)> = m
            .lists
            .iter()
            .filter(|l| l.starts_with("LIST1\t"))
            .map(|l| {
                let p: Vec<&str> = l.split('\t').collect();
                (format!("{} {}", p[2], p[5]), p[6].parse().unwrap())
            })
            .collect();
        assert_eq!(saved_l1, l1, "List 1 at ply {}", m.ply);
        let cands: Vec<&String> = m.lists.iter().filter(|l| l.starts_with("CAND\tsearched\t")).collect();
        assert_eq!(cands.len(), l3.len());
        for (uci, rows) in &l3 {
            let saved = m.lists.iter().filter(|l| l.starts_with(&format!("LIST3\tsearched {uci}\t"))).count();
            assert_eq!(saved, *rows, "List 3 after {uci}");
        }
        let marked: Vec<&String> = cands.iter().copied().filter(|l| l.ends_with("\tPLAYED")).collect();
        assert_eq!(marked.len(), 1);
        assert!(marked[0].starts_with(&format!("CAND\tsearched\t{played}\t")));
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn games_accumulate_and_n_changes_no_saved_byte() {
    let dir = temp_dir("accumulate");
    let (first, _) = record_game(&dir, ENDGAME, 2, None);
    let snapshot = fs::read(&first).unwrap();
    let (second, _) = record_game(&dir, ENDGAME, 3, None);
    assert_ne!(first, second);
    assert_eq!(fs::read(&first).unwrap(), snapshot);
    assert_eq!(archive::read_archive(&archive_dir(&dir)).iter().map(|g| g.id).collect::<Vec<_>>(), vec![1, 2]);

    // A new game clears the four working files and not the archive.
    let before = all_bytes(&archive_dir(&dir));
    let mut files = GameFiles::new(&dir).unwrap();
    files.start_game(&Board::from_fen(ENDGAME).unwrap(), &[]).unwrap();
    files.wait_for_searches();
    assert!(fs::read_to_string(dir.join("white_history.txt")).unwrap().contains("No White turns"));
    assert_eq!(all_bytes(&archive_dir(&dir)), before);
    let _ = fs::remove_dir_all(&dir);
}

fn all_bytes(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push((p.clone(), fs::read(&p).unwrap()));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn a_killed_game_gets_game_stopped_and_earlier_bytes_stay() {
    let dir = temp_dir("killed");
    let ai = archive_dir(&dir);
    // A game whose program was killed: no end block, and a record cut off mid-write.
    let mut board = Board::from_fen(ENDGAME).unwrap();
    let mut files = GameFiles::new(&dir).unwrap();
    files.start_game(&board, &[]).unwrap();
    let mut a = GameArchive::create(&ai, &board, &[]).unwrap();
    for _ in 0..2 {
        let m = board.legal_moves()[0];
        play_recorded(&mut files, &mut a, &mut board, m);
    }
    let path = a.path().to_path_buf();
    drop(a);
    let full = fs::read(&path).unwrap();
    let cut = full.len() - 300;
    fs::write(&path, &full[..cut]).unwrap();
    let before = fs::read(&path).unwrap();

    let closed = recover(&ai).unwrap();
    assert_eq!(closed, vec![1]);
    let after = fs::read(&path).unwrap();
    assert_eq!(&after[..before.len()], &before[..], "no earlier byte changed");
    let g = read_game(&path).unwrap();
    assert!(matches!(g.status, GameStatus::Stopped { .. }));
    assert_eq!(g.moves.len(), 1, "the cut-off record is not read as a move");
    // A second start changes nothing more.
    assert!(recover(&ai).unwrap().is_empty());
    assert_eq!(fs::read(&path).unwrap(), after);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_memory_list_the_limit_and_the_general_bank() {
    let dir = temp_dir("memory");
    let ai = archive_dir(&dir);
    // 3 games of 20 moves: 60 saved moves, more than the limit.
    for _ in 0..3 {
        record_game(&dir, ENDGAME, 20, Some(Outcome::draw("draw agreement", "Draw by agreement.".into())));
    }
    let all = memories(&ai, false);
    assert_eq!(all.len(), 60);
    assert_eq!(all[0], archive::Memory { game: 1, ply: 1 });
    assert_eq!(all[59], archive::Memory { game: 3, ply: 20 });
    // The limit returns at most 50 and deletes nothing.
    let index_before = fs::read(ai.join(MEMORY_INDEX)).unwrap();
    assert_eq!(memories(&ai, true).len(), MEMORY_LIMIT);
    assert_eq!(fs::read(ai.join(MEMORY_INDEX)).unwrap(), index_before);
    assert_eq!(memories(&ai, false).len(), 60);
    // The general bank holds the finished games.
    assert_eq!(archive::bank(&ai, "general"), all);
    // A game being recorded is absent until it ends.
    let board = Board::from_fen(ENDGAME).unwrap();
    let mut files = GameFiles::new(&dir).unwrap();
    files.start_game(&board, &[]).unwrap();
    let mut open = GameArchive::create(&ai, &board, &[]).unwrap();
    let mut b = board;
    let m = b.legal_moves()[0];
    play_recorded(&mut files, &mut open, &mut b, m);
    assert!(memories(&ai, false).iter().all(|x| x.game != 4));
    open.stop("the test ended").unwrap();
    assert!(memories(&ai, false).iter().any(|x| x.game == 4));
    // Rebuilding gives the same list and bank, and changes no archive file.
    let games_before: Vec<Vec<u8>> = archive::archive_files(&ai).iter().map(|(_, p)| fs::read(p).unwrap()).collect();
    let index = fs::read(ai.join(MEMORY_INDEX)).unwrap();
    let bank = fs::read(ai.join("banks").join("general.txt")).unwrap();
    fs::remove_file(ai.join(MEMORY_INDEX)).unwrap();
    rebuild_memory(&ai).unwrap();
    assert_eq!(fs::read(ai.join(MEMORY_INDEX)).unwrap(), index);
    assert_eq!(fs::read(ai.join("banks").join("general.txt")).unwrap(), bank);
    let games_after: Vec<Vec<u8>> = archive::archive_files(&ai).iter().map(|(_, p)| fs::read(p).unwrap()).collect();
    assert_eq!(games_before, games_after);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn format_one_files_still_read() {
    let dir = temp_dir("format1");
    let ai = archive_dir(&dir);
    fs::create_dir_all(&ai).unwrap();
    fs::write(
        ai.join("game_1.txt"),
        "GAME START\nid: 1\nfen: rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1\ntime_unix: 1\nEND GAME START\n\
         MEMORY game=1 ply=1\nmove_number: 1\nside: White\nfen_before: rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1\n\
         record: RECORD White e2 e4 P - - 0 e4\nfen_after: rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1\nLIST1\tWhite\t1:45 5:45\nEND MEMORY\n\
         GAME STOPPED\nreason: the player started another game\ntime_unix: 2\nEND GAME STOPPED\n",
    )
    .unwrap();
    fs::write(ai.join("game_2.txt"), "GAME START\nid: 2\ntime_unix: 3\nEND GAME START\nGAME END\nresult: White resigns. Black wins.\ntime_unix: 4\nEND GAME END\n").unwrap();
    let g1 = fs::read(ai.join("game_1.txt")).unwrap();
    assert!(recover(&ai).unwrap().is_empty());
    assert_eq!(fs::read(ai.join("game_1.txt")).unwrap(), g1);
    let games = archive::read_archive(&ai);
    assert_eq!(games[0].format, 1);
    assert_eq!(games[0].moves[0].san, "e4");
    assert!(matches!(&games[1].status, GameStatus::Finished { result_code, .. } if result_code == "0-1"));
    let (p, _) = record_game(&dir, ENDGAME, 1, None);
    assert!(p.ends_with("game_3.txt"));
    let _ = fs::remove_dir_all(&dir);
}

/// From the starting position the saved future file is the full search:
/// 124,132,536 nodes, strategies read through all six moves.
#[test]
fn from_the_start_the_saved_future_is_the_full_six_move_search() {
    let dir = temp_dir("start");
    let (path, _) = record_game(&dir, START_FEN, 1, None);
    let g = read_game(&path).unwrap();
    let fut = String::from_utf8(g.moves[0].file(&future_name(Color::White)).unwrap().to_vec()).unwrap();
    assert!(fut.contains("Total search nodes: 124,132,536"), "{}", &fut[..fut.len().min(800)]);
    let store: FutureStore = g.moves[0].future_store().unwrap();
    // A strategy first appearing at individual move 5 or 6 can only be seen if
    // moves 5 and 6 were read.
    let deepest = store.entries[""].first.iter().flatten().flatten().copied().max().unwrap();
    assert!(deepest >= 5, "the strategies were read through all six moves (deepest first appearance {deepest})");
    let _ = fs::remove_dir_all(&dir);
}

// ----- The program itself ----------------------------------------------------------

fn run_program(dir: &Path, input: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_chess"))
        .args(["--dir", dir.to_str().unwrap(), "--fen", ENDGAME])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    String::from_utf8_lossy(&child.wait_with_output().unwrap().stdout).into_owned()
}

#[test]
fn the_program_asks_records_and_closes_games() {
    let dir = temp_dir("program");
    let ai = archive_dir(&dir);
    let out = run_program(&dir, "y\ne2\ne4\ny\nd5\nd4\ny\nresign\ny\nquit\n");
    assert!(out.contains("Save this game's information to the permanent AI learning files?\nY — Yes\nN — No"), "{out}");
    assert!(out.contains("saved in the archive with its result (0-1)"), "{out}");
    let g = read_game(&ai.join("game_1.txt")).unwrap();
    assert_eq!(g.moves.len(), 2);
    assert!(matches!(&g.status, GameStatus::Finished { result_code, termination, .. } if result_code == "0-1" && termination == "resignation"));

    // N changes no saved byte.
    let before = all_bytes(&ai);
    let out = run_program(&dir, "n\ne2\ne4\ny\nquit\n");
    assert!(out.contains("This game is not recorded"));
    assert_eq!(all_bytes(&ai), before);

    // Quitting keeps the game, stopped.
    run_program(&dir, "y\ne2\ne3\ny\nquit\n");
    let g = read_game(&ai.join("game_2.txt")).unwrap();
    assert_eq!(g.status, GameStatus::Stopped { reason: "the player quit the program".into() });
    assert_eq!(memories(&ai, false).len(), 3);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn pasted_text_does_not_run_commands_and_new_asks_first() {
    let dir = temp_dir("paste");
    let out = run_program(&dir, "y\ne2\ne4\ny\nNew game. All four game files were cleared.\nLists (Steps 8–13)\nResign, draw\nnew\nn\nquit\n");
    assert_eq!(out.matches("Save this game's information").count(), 1, "{out}");
    assert!(out.contains("Unknown command 'New game. All four game files were cleared.'"));
    assert!(out.contains("The game continues."));
    let g = read_game(&archive_dir(&dir).join("game_1.txt")).unwrap();
    assert_eq!(g.moves.len(), 1);
    let _ = fs::remove_dir_all(&dir);
}
