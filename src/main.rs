//! Console chess program (Steps 1–3), with the four game files (Steps 4–7)
//! and Lists 1–4 (Steps 8–13).
//!
//! Usage:
//!   chess                      play a game (files go to ./game_files)
//!   chess --dir <folder>       put the four game files somewhere else
//!   chess --fen "<fen>"        start from a position
//!   chess perft <depth> [fen]  count legal move sequences (correctness check)
//!   chess search [fen]         run the six-move future search once and print the totals

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use chess::archive::{self as archive, archive_dir, GameArchive, GameStatus, Outcome, WorkingFiles, MEMORY_LIMIT, RECORD_QUESTION};
use chess::board::{parse_square, square_name, Board, Color, Move, PieceKind, START_FEN};
use chess::evaluate::{
    describe_capture, evaluate_stored, format_material, format_signed, list3_after_stored, list3_all_stored, list4_after_stored,
    list4_all_stored, Evaluation,
};
use chess::files::GameFiles;
use chess::position::RecordedMove;
use chess::presets::{preset_by_id, presets};
use chess::search::{full_search, mover_at, thousands, FutureStore, Progress, SEARCH_PLIES};
use chess::strategies::STRATEGIES;

const USAGE: &str = "\
Usage:
  chess                      play a game (files go to ./game_files)
  chess --dir <folder>       put the four game files somewhere else
  chess --fen \"<fen>\"        start from a position
  chess perft <depth> [fen]  count legal move sequences (correctness check)
  chess search [fen]         run the six-move future search once and print the totals";

struct Game {
    board: Board,
    moves: Vec<RecordedMove>,
    keys: Vec<String>,
    files: GameFiles,
    /// Set when a player resigns or both agree to a draw.
    ended: Option<Outcome>,
    /// The permanent archive file of this game, when the player chose to record it.
    archive: Option<GameArchive>,
}

enum Input {
    Line(String),
    Eof,
}

fn read_line(prompt: &str) -> Input {
    print!("{prompt}");
    let _ = io::stdout().flush();
    let mut line = String::new();
    match io::stdin().lock().read_line(&mut line) {
        Ok(0) | Err(_) => Input::Eof,
        Ok(_) => Input::Line(line.trim().to_string()),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut dir = PathBuf::from("game_files");
    let mut fen = START_FEN.to_string();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "perft" => return run_perft(&args[i + 1..]),
            "search" => return run_search(&args[i + 1..]),
            "--dir" if i + 1 < args.len() => {
                dir = PathBuf::from(&args[i + 1]);
                i += 1;
            }
            "--fen" if i + 1 < args.len() => {
                fen = args[i + 1].clone();
                i += 1;
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                return;
            }
            other => {
                eprintln!("Unknown argument '{other}'. Try --help.");
                std::process::exit(2);
            }
        }
        i += 1;
    }

    let board = match Board::from_fen(&fen) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Could not read the FEN: {e}");
            std::process::exit(2);
        }
    };
    if board.in_check(board.turn().other()) {
        eprintln!("Could not use that FEN: the side not to move is in check.");
        std::process::exit(2);
    }
    let files = match GameFiles::new(&dir) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Could not create the folder {}: {e}", dir.display());
            std::process::exit(1);
        }
    };
    let mut game = Game { board, moves: vec![], keys: vec![], files, ended: None, archive: None };
    println!("Forming Strategies chess. Type 'help' for commands.");
    println!("Game files are in {}", game.files.dir().display());
    match archive::recover(&archive_dir(game.files.dir())) {
        Ok(closed) => {
            for id in closed {
                println!("Game {id} in the archive was left open when the program last stopped. GAME STOPPED was added to its end; nothing earlier changed.");
            }
        }
        Err(e) => println!("Warning: could not check the archive for games left open: {e}"),
    }
    game.new_game(board, vec![]);
    game.run();
}

impl Game {
    /// Start a game. Any recorded game must be closed first (see `close_archive`).
    fn new_game(&mut self, board: Board, moves: Vec<RecordedMove>) {
        self.board = board;
        self.moves = moves;
        self.keys = vec![board.position_key()];
        self.ended = None;
        let record = ask_record();
        if let Err(e) = self.files.start_game(&board, &self.moves) {
            println!("Warning: could not write the game files: {e}");
        }
        self.archive = None;
        if record {
            match GameArchive::create(&archive_dir(self.files.dir()), &board, &self.moves) {
                Ok(a) => {
                    println!("Recording this game as game {} ({}).", a.id(), a.path().display());
                    self.archive = Some(a);
                }
                Err(e) => {
                    println!("Warning: could not create the archive file, so this game is not recorded: {e}");
                }
            }
        } else {
            println!("This game is not recorded. The permanent archive is not changed.");
        }
    }

    /// The stored Step 6 search of the current position, which every list reads.
    /// Waits for it to finish; a list never starts a search of its own.
    fn stored(&mut self) -> Option<FutureStore> {
        if self.files.search_running_for(self.board.turn()) {
            println!("Waiting for the future search of this position to finish ({})…", self.files.search_status());
        }
        let store = self.files.stored_search(&self.board);
        if store.is_none() {
            println!("The future search for this position is not available (it was cancelled or the game files could not be written).");
        }
        store
    }

    fn outcome(&self) -> Option<Outcome> {
        let b = &self.board;
        if let Some(o) = &self.ended {
            return Some(o.clone());
        }
        if b.is_checkmate() {
            let winner = b.turn().other();
            return Some(Outcome::win(winner, "checkmate", format!("Checkmate. {} wins.", winner.name())));
        }
        if b.is_stalemate() {
            return Some(Outcome::draw("stalemate", "Stalemate. The game is drawn.".into()));
        }
        if b.is_insufficient_material() {
            return Some(Outcome::draw("insufficient material", "Draw by insufficient material.".into()));
        }
        if b.is_fifty_move_draw() {
            return Some(Outcome::draw("fifty-move rule", "Draw by the fifty-move rule.".into()));
        }
        let key = b.position_key();
        if self.keys.iter().filter(|k| **k == key).count() >= 3 {
            return Some(Outcome::draw("threefold repetition", "Draw by threefold repetition.".into()));
        }
        None
    }

    fn game_over(&self) -> Option<String> {
        self.outcome().map(|o| o.text)
    }

    /// Close the recorded game with its result, or as stopped (15.4).
    fn close_archive(&mut self, outcome: Option<&Outcome>, reason: &str) {
        let Some(mut a) = self.archive.take() else { return };
        let result = match outcome {
            Some(o) => a.finish(o),
            None => a.stop(reason),
        };
        match result {
            Ok(()) => match outcome {
                Some(o) => println!("Game {} is saved in the archive with its result ({}).", a.id(), o.code),
                None => println!("Game {} is saved in the archive as stopped: {reason}.", a.id()),
            },
            Err(e) => println!("Warning: could not finish writing the archive file {}: {e}", a.path().display()),
        }
    }

    /// On Y, the Step 6 search of the current position finishes before the move
    /// is confirmed, and its progress is shown while the program waits (15.3).
    fn wait_for_search_before_confirming(&mut self) {
        if self.archive.is_none() {
            return;
        }
        let turn = self.board.turn();
        if self.files.search_running_for(turn) {
            println!("This game is recorded, so the six-move future search of this position finishes before the move is confirmed.");
            while self.files.search_running_for(turn) {
                print!("\r  {}      ", self.files.search_status());
                let _ = io::stdout().flush();
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
            println!("\r  {}      ", self.files.search_status());
        }
        // Join the finished search so its file is complete on disk.
        if self.files.stored_search(&self.board).is_none() {
            println!("Warning: the future search of this position could not be finished, so its saved future-search file is incomplete.");
        }
    }

    fn show_board(&self) {
        println!();
        print!("{}", self.board.ascii());
        let last = self.moves.last().map(|m| format!("  Last move: {}", m.san)).unwrap_or_default();
        println!("  Move {}. {} to move.{}", self.board.fullmove_number(), self.board.turn().name(), last);
        if self.board.is_check() && !self.board.is_checkmate() {
            println!("  {} is in check.", self.board.turn().name());
        }
    }

    fn run(&mut self) {
        self.show_board();
        loop {
            if let Some(over) = self.outcome() {
                if self.archive.as_ref().is_some_and(|a| !a.is_closed()) {
                    self.close_archive(Some(&over), "");
                }
                println!("\n{} Type 'new' for a new game or 'quit' to exit.", over.text);
            }
            let prompt = format!("\n{} — select a piece by its square (e.g. e2), or type a command: ", self.board.turn().name());
            let line = match read_line(&prompt) {
                Input::Eof => break,
                Input::Line(l) => l,
            };
            if line.is_empty() {
                continue;
            }
            if let Some(sq) = parse_square(&line) {
                if self.game_over().is_some() {
                    println!("The game is over. Type 'new' to start another.");
                    continue;
                }
                if !self.select_piece(sq) {
                    break;
                }
                continue;
            }
            if !self.command(&line) {
                break;
            }
        }
        self.close_archive(None, "the player quit the program");
        println!("Stopping the future search…");
        self.files.stop_workers();
        println!("Goodbye.");
    }

    // ----- Steps 1–3 ----------------------------------------------------------

    /// Returns false when input ended.
    fn select_piece(&mut self, from: u8) -> bool {
        let Some(piece) = self.board.piece_at(from) else {
            println!("There is no piece on {}.", square_name(from));
            return true;
        };
        let turn = self.board.turn();
        if piece.color != turn {
            println!(
                "The {} on {} belongs to {}. It is {}'s turn.",
                piece.kind.name(),
                square_name(from),
                piece.color.name(),
                turn.name()
            );
            return true;
        }
        let legal = self.board.legal_moves();
        let mine: Vec<Move> = legal.iter().filter(|m| m.from == from).copied().collect();

        // Step 2
        if mine.is_empty() {
            let why = if self.board.is_check() {
                format!(" {} is in check, and this piece cannot help.", turn.name())
            } else {
                String::new()
            };
            println!(
                "The {} on {} cannot currently make a legal move.{} Select another piece.",
                piece.kind.name(),
                square_name(from),
                why
            );
            return true;
        }

        // Step 1
        let mut destinations: Vec<u8> = mine.iter().map(|m| m.to).collect();
        destinations.dedup();
        let shown: Vec<String> = destinations
            .iter()
            .map(|&to| {
                let first = mine.iter().find(|m| m.to == to).unwrap();
                let san = self.board.san_with(first, &legal);
                let plain = san.trim_end_matches(['+', '#']);
                if first.promotion.is_some() {
                    format!("{} (promotes)", square_name(to))
                } else if plain == square_name(to) && san == plain {
                    square_name(to)
                } else {
                    format!("{} ({})", square_name(to), san)
                }
            })
            .collect();
        println!(
            "{} {} on {} — {} legal move{}: {}",
            turn.name(),
            piece.kind.name(),
            square_name(from),
            destinations.len(),
            if destinations.len() == 1 { "" } else { "s" },
            shown.join(", ")
        );

        // Step 3
        loop {
            let line = match read_line("Destination square (Enter to choose a different piece): ") {
                Input::Eof => return false,
                Input::Line(l) => l,
            };
            if line.is_empty() {
                return true;
            }
            let Some(to) = parse_square(&line) else {
                println!("Enter a square as a letter and a number, such as e4.");
                continue;
            };
            let options: Vec<Move> = mine.iter().filter(|m| m.to == to).copied().collect();
            if options.is_empty() {
                println!("The {} on {} cannot move to {}.", piece.kind.name(), square_name(from), square_name(to));
                continue;
            }
            let chosen = if options.len() > 1 {
                match self.ask_promotion(&options) {
                    Some(m) => m,
                    None => return true,
                }
            } else {
                options[0]
            };
            let san = self.board.san_with(&chosen, &legal);
            self.wait_for_search_before_confirming();
            let question = format!(
                "Move the {} from {} to {} ({san})? Confirm with Y or cancel with N: ",
                piece.kind.name(),
                square_name(from),
                square_name(to)
            );
            loop {
                match read_line(&question) {
                    Input::Eof => return false,
                    Input::Line(answer) => match answer.to_ascii_uppercase().as_str() {
                        "Y" => {
                            self.play(chosen, san);
                            return true;
                        }
                        "N" => {
                            println!(
                                "Move cancelled. The {} stays on {}. Select the same piece or a different one.",
                                piece.kind.name(),
                                square_name(from)
                            );
                            return true;
                        }
                        _ => println!("Please enter Y or N."),
                    },
                }
            }
        }
    }

    fn ask_promotion(&self, options: &[Move]) -> Option<Move> {
        loop {
            match read_line("Promote to which piece? Q, R, B or N (Enter to cancel): ") {
                Input::Eof => return None,
                Input::Line(l) if l.is_empty() => return None,
                Input::Line(l) => {
                    let kind = l.chars().next().and_then(PieceKind::from_letter);
                    if let Some(m) = options.iter().find(|m| m.promotion.is_some() && m.promotion == kind) {
                        return Some(*m);
                    }
                    println!("Enter Q, R, B or N.");
                }
            }
        }
    }

    fn play(&mut self, m: Move, san: String) {
        let before = self.board;
        let lists_past = self.files.history_moves();
        let mut record = RecordedMove::new(&before, &m);
        record.san = san.clone();
        self.board.make(&m);
        self.moves.push(record);
        self.keys.push(self.board.position_key());
        let after = self.board;
        let recording = self.archive.is_some();
        // Step 15.5: copy the four working files after the history files hold
        // this move and before the next future search starts.
        match self.files.record_turn_then(&before, &m, &san, &after, |f| recording.then(|| WorkingFiles::read(f))) {
            Ok(copy) => {
                if let (Some(a), Some(copy)) = (self.archive.as_mut(), copy) {
                    let saved = copy.and_then(|files| a.record_move(&before, &m, &san, &after, &lists_past, &files));
                    if let Err(e) = saved {
                        println!("Warning: could not write to the archive file: {e}");
                    }
                }
            }
            Err(e) => println!("Warning: could not write the game files: {e}"),
        }
        self.show_board();
        println!("  The future search of this position is running in the background. Type 'lists' for List 1 and List 2.");
    }

    // ----- Commands ---------------------------------------------------------

    /// Returns false to quit.
    fn command(&mut self, line: &str) -> bool {
        let mut parts = line.split_whitespace();
        let cmd = parts.next().unwrap_or("").to_ascii_lowercase();
        let rest: Vec<&str> = parts.collect();
        // Commands are matched exactly, so text pasted into the game (such as
        // "New game. All four game files were cleared.") never runs one.
        const NO_ARGUMENTS: [&str; 18] = [
            "help", "?", "quit", "exit", "q", "board", "b", "resign", "draw", "moves", "list3", "strategies", "files", "status",
            "wait", "new", "fen", "presets",
        ];
        let side_word = |w: &str| matches!(w.to_ascii_lowercase().as_str(), "w" | "b" | "white" | "black");
        let exact = if NO_ARGUMENTS.contains(&cmd.as_str()) {
            rest.is_empty()
        } else if matches!(cmd.as_str(), "lists" | "list1" | "list2" | "l" | "history") {
            rest.len() <= 1 && rest.iter().all(|w| side_word(w))
        } else if matches!(cmd.as_str(), "list4" | "strategy" | "preset") {
            rest.len() <= 1
        } else if cmd == "future" || cmd == "f" {
            rest.len() <= 2
        } else if cmd == "archive" {
            rest.len() <= 2 && rest.iter().all(|w| w.parse::<u32>().is_ok())
        } else {
            true
        };
        if !exact {
            println!("Unknown command '{}'. Type 'help' for the list, or enter a square such as e2.", line.trim());
            return true;
        }
        match cmd.as_str() {
            "help" | "?" => print_help(),
            "quit" | "exit" | "q" => return false,
            "board" | "b" => self.show_board(),
            "resign" => {
                if self.game_over().is_some() {
                    println!("The game is already over.");
                } else if let Input::Line(a) = read_line(&format!("{} resigns? Confirm with Y or cancel with N: ", self.board.turn().name())) {
                    if a.eq_ignore_ascii_case("y") {
                        let (loser, winner) = (self.board.turn(), self.board.turn().other());
                        self.ended = Some(Outcome::win(winner, "resignation", format!("{} resigns. {} wins.", loser.name(), winner.name())));
                    }
                }
            }
            "draw" => {
                if self.game_over().is_some() {
                    println!("The game is already over.");
                } else if let Input::Line(a) = read_line(&format!(
                    "{} offers a draw. Does {} accept? Y or N: ",
                    self.board.turn().name(),
                    self.board.turn().other().name()
                )) {
                    if a.eq_ignore_ascii_case("y") {
                        self.ended = Some(Outcome::draw("draw agreement", "Draw by agreement.".into()));
                    } else {
                        println!("Draw declined. The game continues.");
                    }
                }
            }
            "moves" => {
                let legal = self.board.legal_moves();
                let sans: Vec<String> = legal.iter().map(|m| self.board.san_with(m, &legal)).collect();
                println!("{} legal moves: {}", sans.len(), sans.join(" "));
            }
            "lists" | "list1" | "list2" | "l" => {
                let side = rest.first().and_then(|s| s.chars().next()).and_then(Color::from_letter).unwrap_or(self.board.turn());
                if let Some(store) = self.stored() {
                    let past = self.files.history_moves();
                    let e = evaluate_stored(&self.board, side, &past, &store, &[]);
                    print_list1(&e, "List 1");
                    print_list2(&e, "List 2");
                }
            }
            "list3" => self.cmd_list3(),
            "list4" => self.cmd_list4(&rest),
            "future" | "f" => self.cmd_future(&rest),
            "strategies" => {
                for (i, s) in STRATEGIES.iter().enumerate() {
                    println!("{:>2}. {} — {}", i + 1, s.name, s.intent);
                }
                println!("Type 'strategy <number>' for its conditions.");
            }
            "strategy" => match rest.first().and_then(|n| n.parse::<usize>().ok()).filter(|n| (1..=20).contains(n)) {
                Some(n) => {
                    let s = &STRATEGIES[n - 1];
                    println!("{}. {}\n  Intent:      {}\n  Foundation:  {}\n  Progress:    {}\n  Completion:  {}\n  Captures:    {}",
                        n, s.name, s.intent, s.foundation, s.progress, s.completion, s.capture_link);
                }
                None => println!("Usage: strategy <1-20>"),
            },
            "files" => {
                for p in self.files.all_paths() {
                    println!("  {}", p.display());
                }
                println!("  {}", self.files.search_status());
            }
            "status" => println!("{}", self.files.search_status()),
            "wait" => {
                println!("Waiting for the future search to finish…");
                self.files.wait_for_searches();
                println!("{}", self.files.search_status());
            }
            "history" => {
                let side = rest.first().and_then(|s| s.chars().next()).and_then(Color::from_letter).unwrap_or(self.board.turn().other());
                match std::fs::read_to_string(self.files.history_path(side)) {
                    Ok(t) => println!("{t}"),
                    Err(e) => println!("Could not read the file: {e}"),
                }
            }
            "new" => {
                if self.keys.len() > 1 && self.game_over().is_none() && !confirm("Start a new game? This game stops here. Confirm with Y or cancel with N: ") {
                    println!("The game continues.");
                    return true;
                }
                self.close_archive(None, "the player started another game");
                self.new_game(Board::start(), vec![]);
                println!("New game. All four game files were cleared.");
                self.show_board();
            }
            "fen" => println!("{}", self.board.fen()),
            "load" => match Board::from_fen(&rest.join(" ")) {
                Ok(b) if b.in_check(b.turn().other()) => {
                    println!("Could not use that FEN: {} is in check but it is not {}'s turn.", b.turn().other().name(), b.turn().other().name());
                }
                Ok(b) => {
                    self.close_archive(None, "the player loaded another position");
                    self.new_game(b, vec![]);
                    println!("Position loaded as a new game. All four game files were cleared.");
                    self.show_board();
                }
                Err(e) => println!("Could not read that FEN: {e}"),
            },
            "presets" => {
                for p in presets() {
                    println!("  {:<17} {:<11} {}", p.id, p.group, p.name);
                }
                println!("Type 'preset <id>' to load one.");
            }
            "preset" => match rest.first().and_then(|id| preset_by_id(id)) {
                Some(p) => {
                    self.close_archive(None, "the player loaded a preset position");
                    self.new_game(p.board(), p.moves.clone());
                    println!("{}: {}", p.name, p.detail);
                    self.show_board();
                }
                None => println!("Unknown preset. Type 'presets' for the list."),
            },
            "archive" => self.cmd_archive(&rest),
            _ => println!("Unknown command '{cmd}'. Type 'help' for the list, or enter a square such as e2."),
        }
        true
    }

    /// The permanent archive: every recorded game, one game's moves, or one move.
    fn cmd_archive(&mut self, rest: &[&str]) {
        let dir = archive_dir(self.files.dir());
        let current = self.archive.as_ref().map(|a| a.id());
        let nums: Vec<u32> = rest.iter().filter_map(|w| w.parse().ok()).collect();
        let games = archive::read_archive(&dir);
        match nums.as_slice() {
            [] => {
                println!("Permanent AI learning archive: {} recorded game{} in {}", games.len(), if games.len() == 1 { "" } else { "s" }, dir.display());
                for g in &games {
                    let status = match &g.status {
                        _ if Some(g.id) == current => "being recorded now".to_string(),
                        GameStatus::Finished { result_code, termination, .. } => {
                            format!("GAME END {result_code}{}", if termination.is_empty() { String::new() } else { format!(" ({termination})") })
                        }
                        GameStatus::Stopped { reason } => format!("GAME STOPPED ({reason})"),
                        GameStatus::Open => "open".to_string(),
                    };
                    println!("  game {:<4} format {}  {:<45} {} saved move{}", g.id, g.format, status, g.moves.len(), if g.moves.len() == 1 { "" } else { "s" });
                }
                let all = archive::memories(&dir, false).len();
                let limited = archive::memories(&dir, true).len();
                let general = archive::bank(&dir, "general").len();
                println!("Memory list: {all} saved moves from games with an end block; a limited read returns {limited} (limit {MEMORY_LIMIT}). General bank: {general} references.");
                println!("Saving is not training: no neural network has been trained, so no AI advice is shown beside Lists 1 and 3.");
                println!("Type 'archive <game>' for its moves, or 'archive <game> <ply>' for one saved move.");
            }
            [id] => match games.iter().find(|g| g.id == *id) {
                Some(g) => {
                    println!("Game {} ({}):", g.id, g.path.display());
                    for m in &g.moves {
                        let parts = if g.format >= 3 {
                            format!("past, present and future saved ({} working files, {} list lines)", m.files.files.len(), m.lists.len())
                        } else {
                            "format 1: summary only".to_string()
                        };
                        println!("  ply {:<4} {:<6} {:<8} {}", m.ply, m.side.name(), m.san, parts);
                    }
                    if g.moves.is_empty() {
                        println!("  No moves were played.");
                    }
                }
                None => println!("There is no archived game {id}."),
            },
            [id, ply, ..] => match games.iter().find(|g| g.id == *id).and_then(|g| g.moves.iter().find(|m| m.ply == *ply).map(|m| (g, m))) {
                Some((g, m)) => {
                    println!("Game {} ply {}: {} {} ({})", g.id, m.ply, m.side.name(), m.san, m.uci);
                    println!("fen_before: {}\nfen_after:  {}", m.fen_before, m.fen_after);
                    for (name, bytes) in &m.files.files {
                        let text = String::from_utf8_lossy(bytes);
                        if name.ends_with("_history.txt") {
                            println!("\n----- {name} ({} bytes, saved unchanged) -----\n{text}", bytes.len());
                        } else {
                            let lines = text.lines().count();
                            println!("\n----- {name} ({} bytes, {lines} lines, saved unchanged; first 16 lines) -----", bytes.len());
                            for l in text.lines().take(16) {
                                println!("{l}");
                            }
                        }
                    }
                    println!("\n----- Lists saved with this move ({} lines) -----", m.lists.len());
                    for l in &m.lists {
                        println!("{l}");
                    }
                    println!("\nThe whole record is in {}.", g.path.display());
                }
                None => println!("Game {id} has no saved move {ply}."),
            },
        }
    }

    fn cmd_list3(&mut self) {
        let side = self.board.turn();
        let Some(store) = self.stored() else { return };
        let past = self.files.history_moves();
        let branches = list3_all_stored(&self.board, side, &past, &store);
        if branches.is_empty() {
            println!("There are no legal moves.");
            return;
        }
        println!(
            "List 3 — {}'s forming strategies after each of its {} legal next moves (top three per move)",
            side.name(),
            branches.len()
        );
        for b in &branches {
            let top: Vec<String> = b.evaluation.hits.iter().take(3).map(|h| format!("{} {}", h.name, h.score)).collect();
            println!("  {:<8} {}", b.san, if top.is_empty() { "(none)".to_string() } else { top.join(", ") });
        }
        println!("Type 'future <move>' for one move in full, or 'future <move> <reply>' for List 4.");
    }

    /// List 4 for every legal reply to one List 3 move, or for every List 3 move.
    fn cmd_list4(&mut self, rest: &[&str]) {
        let side = self.board.turn();
        let Some(store) = self.stored() else { return };
        let past = self.files.history_moves();
        let branches = match rest.first() {
            Some(m) if !m.eq_ignore_ascii_case("all") => match list3_after_stored(&self.board, side, &past, m, &store) {
                Some(b) => vec![b],
                None => {
                    println!("'{m}' is not a legal move here. Type 'moves' to see them.");
                    return;
                }
            },
            _ => list3_all_stored(&self.board, side, &past, &store),
        };
        if branches.is_empty() {
            println!("List 4 is computed only on {}'s turn, and there are no legal moves.", side.name());
            return;
        }
        println!("List 4 — {}'s strategy captures after each next move and each reply (rows with captures only)", side.name());
        let single = branches.len() == 1;
        for branch in &branches {
            let mut printed = 0;
            let replies = list4_all_stored(branch, &store);
            let total = replies.len();
            for (reply_san, e) in replies {
                if e.captures.is_empty() {
                    continue;
                }
                printed += 1;
                if single {
                    print_list2(&e, &format!("List 4 (after {} {})", branch.san, reply_san));
                } else {
                    let best = &e.captures[0];
                    println!(
                        "  {:<8} {:<8} {} capture row{}; best {} net {}",
                        branch.san,
                        reply_san,
                        e.captures.len(),
                        if e.captures.len() == 1 { "" } else { "s" },
                        best.san,
                        format_signed(best.net)
                    );
                }
            }
            if single && printed == 0 {
                println!("  No reply to {} leaves a strategy capture ({} replies checked).", branch.san, total);
            }
        }
        if !single {
            println!("Type 'list4 <move>' for one move's captures in full.");
        }
    }

    fn cmd_future(&mut self, rest: &[&str]) {
        let side = self.board.turn();
        let Some(next) = rest.first() else {
            println!("Usage: future <move> [reply], for example: future e4  or  future e4 e5");
            return;
        };
        let Some(store) = self.stored() else { return };
        let past = self.files.history_moves();
        let Some(branch) = list3_after_stored(&self.board, side, &past, next, &store) else {
            println!("'{next}' is not a legal move here. Type 'moves' to see them.");
            return;
        };
        print_list1(&branch.evaluation, &format!("List 3 (after {})", branch.san));
        match rest.get(1) {
            None => {
                let legal = branch.board.legal_moves();
                let replies: Vec<String> = legal.iter().map(|m| branch.board.san_with(m, &legal)).collect();
                println!("\n{}'s possible replies: {}", side.other().name(), replies.join(" "));
                println!("Add a reply for List 4, for example: future {} {}", branch.san, replies.first().cloned().unwrap_or_default());
            }
            Some(reply) => match list4_after_stored(&branch, reply, &store) {
                Some((reply_san, e)) => print_list2(&e, &format!("List 4 (after {} {})", branch.san, reply_san)),
                None => println!("'{reply}' is not a legal reply after {}.", branch.san),
            },
        }
    }
}

fn print_help() {
    println!(
        "\
Playing (Steps 1–3)
  e2              select the piece on a square and see its legal moves,
                  then enter a destination and confirm with Y or N
Lists (Steps 8–13)
  lists [w|b]     List 1 (forming strategies) and List 2 (strategy captures)
  list3           List 3 for every legal next move
  future <move>   List 3 after one next move, with the possible replies
  future <move> <reply>   List 4: captures after the move and that reply
  list4 [move|all]        List 4 for every reply to one move, or to every move
                  (Lists 1-4 read the stored six-move future search; they never start one)
  strategies      the 20 strategies; 'strategy <n>' shows one in full
Files (Steps 4–7)
  files           where the four files are, and the future-search status
  history [w|b]   print a history file
  wait            wait for the six-move future search to finish
  status          progress of the two future searches
Archive
  archive         every recorded game; 'archive <game>' lists its moves;
                  'archive <game> <ply>' shows one saved move
Other
  resign, draw (offer a draw), board, moves, fen, load <fen>, presets, preset <id>, new, quit"
    );
}

/// Y or N; end of input counts as N.
fn confirm(prompt: &str) -> bool {
    loop {
        match read_line(prompt) {
            Input::Eof => return false,
            Input::Line(a) => match a.to_ascii_uppercase().as_str() {
                "Y" => return true,
                "N" => return false,
                _ => println!("Please enter Y or N."),
            },
        }
    }
}

/// The question at the start of every game (rules, permanent archive A).
fn ask_record() -> bool {
    println!("\n{RECORD_QUESTION}");
    confirm("Y or N: ")
}

fn print_list1(e: &Evaluation, title: &str) {
    println!("\n{title} — {}'s forming strategies, most developed first", e.side.name());
    if e.hits.is_empty() {
        println!("  No strategy is forming.");
        return;
    }
    for (i, h) in e.hits.iter().enumerate() {
        println!("  {:>2}. {:<30} {:>3}  {}", i + 1, h.name, h.score, h.stage.label());
        println!("      {}", h.evidence);
        if h.stage != chess::evaluate::Stage::Full {
            let mut reach = Vec::new();
            if let Some(n) = h.progress_in.filter(|_| !h.progress) {
                reach.push(format!("progress by move {n}"));
            }
            if let Some(n) = h.completion_in {
                reach.push(format!("completion by move {n}"));
            }
            if !reach.is_empty() {
                println!("      Earliest in the stored six-move search: {}.", reach.join(", "));
            }
        }
    }
}

fn print_list2(e: &Evaluation, title: &str) {
    println!("\n{title} — {}'s strategy captures, most valuable first", e.side.name());
    if !e.side_to_move {
        println!(
            "  Empty: it is {}'s turn. Captures are only listed for the player to move (rule 14.1.3).",
            e.turn.name()
        );
        return;
    }
    if e.captures.is_empty() {
        println!("  No legal capture belongs to a forming strategy.");
    }
    for (i, c) in e.captures.iter().enumerate() {
        println!(
            "  {:>2}. {:<8} {} {} takes {} {}   gain {}  loss {}  net {}   [{}]",
            i + 1,
            c.san,
            c.capturer.name(),
            square_name(c.from),
            c.victim.name(),
            square_name(c.victim_square),
            format_material(c.expected_gain),
            format_material(c.potential_loss),
            format_signed(c.net),
            c.strategy_name
        );
        println!("      {}", describe_capture(c));
    }
    let unclaimed: Vec<String> = e
        .legal_captures
        .iter()
        .filter(|c| c.strategies.is_empty())
        .map(|c| format!("{} ({})", c.san, format_signed(c.valuation.net)))
        .collect();
    if !unclaimed.is_empty() {
        println!("  Other legal captures, not tied to a strategy: {}", unclaimed.join(", "));
    }
}

fn run_perft(args: &[String]) {
    let depth: u32 = args.first().and_then(|d| d.parse().ok()).unwrap_or(SEARCH_PLIES as u32);
    let fen = if args.len() > 1 { args[1..].join(" ") } else { START_FEN.to_string() };
    let board = Board::from_fen(&fen).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2)
    });
    for d in 1..=depth {
        let t = std::time::Instant::now();
        let n = chess::board::perft(&board, d);
        println!("depth {d}: {:>15}  ({:.2?})", thousands(n), t.elapsed());
    }
}

fn run_search(args: &[String]) {
    let fen = if args.is_empty() { START_FEN.to_string() } else { args.join(" ") };
    let board = Board::from_fen(&fen).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2)
    });
    println!("Searching every legal sequence to six individual moves…");
    let report = full_search(&board, &[], &Progress::default()).expect("not cancelled");
    for d in 0..SEARCH_PLIES {
        println!(
            "  Individual move {} ({}): {:>15} possible sequences",
            d + 1,
            mover_at(report.to_move, d).name(),
            thousands(report.counts[d])
        );
    }
    println!("  Total search nodes: {}", thousands(report.total_nodes()));
    println!("  Took {:.2} s on {} thread(s).", report.elapsed.as_secs_f64(), report.threads);
}
