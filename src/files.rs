//! The four game files (Steps 4, 5, 6 and 7).
//!
//! * `white_history.txt` / `black_history.txt` – for each of that player's
//!   previous four turns and the current turn, every piece's available legal
//!   moves before and after the move was finalized. Each turn also carries a
//!   machine-readable `RECORD` line; the strategy window (the past) is read back
//!   from these files.
//! * `white_future.txt` / `black_future.txt` – the three-full-turn search from
//!   the latest position where that player is the next to move, with the
//!   strategy evidence that Lists 1, 3 and 4 read (the future).
//!
//! All four files are cleared at the start of every game. History files are
//! rewritten after every confirmed move. Future-search files are rebuilt in a
//! background thread so the game never waits for the search.

use std::collections::VecDeque;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread::JoinHandle;

use crate::board::{parse_square, square_name, Board, Color, Move, PieceKind};
use crate::position::RecordedMove;
use crate::search::{full_search, mover_at, thousands, FutureStore, Progress, SearchReport, LISTED_PLIES, SEARCH_PLIES};

/// How many of a player's turns each history file keeps: four previous plus the current one.
pub const HISTORY_TURNS: usize = 5;

#[derive(Clone, Debug)]
pub struct TurnRecord {
    pub move_number: u32,
    pub color: Color,
    pub san: String,
    pub uci: String,
    pub before: String,
    pub after: String,
    pub mv: RecordedMove,
}

struct Worker {
    progress: Arc<Progress>,
    handle: Option<JoinHandle<()>>,
}

impl Worker {
    fn finished(&self) -> bool {
        self.handle.as_ref().is_none_or(|h| h.is_finished())
    }
    fn stop(mut self) {
        self.progress.cancel.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

pub struct GameFiles {
    dir: PathBuf,
    history: [VecDeque<TurnRecord>; 2],
    workers: [Option<Worker>; 2],
}

impl GameFiles {
    pub fn new(dir: impl AsRef<Path>) -> io::Result<GameFiles> {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir)?;
        Ok(GameFiles { dir, history: [VecDeque::new(), VecDeque::new()], workers: [None, None] })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn history_path(&self, color: Color) -> PathBuf {
        self.dir.join(format!("{}_history.txt", color.name().to_lowercase()))
    }

    pub fn future_path(&self, color: Color) -> PathBuf {
        self.dir.join(format!("{}_future.txt", color.name().to_lowercase()))
    }

    pub fn all_paths(&self) -> Vec<PathBuf> {
        Color::BOTH
            .iter()
            .flat_map(|&c| [self.history_path(c), self.future_path(c)])
            .collect()
    }

    /// Step 7: clear all four files and generate what the new game needs.
    /// `seed` is the move record of a loaded reference position (empty for a new game).
    pub fn start_game(&mut self, board: &Board, seed: &[RecordedMove]) -> io::Result<()> {
        self.stop_workers();
        self.history = [VecDeque::new(), VecDeque::new()];
        for rec in seed {
            self.history[rec.color.index()].push_back(TurnRecord {
                move_number: 0,
                color: rec.color,
                san: rec.san.clone(),
                uci: String::new(),
                before: "(reference position: legal moves were not recorded)\n".into(),
                after: "(reference position: legal moves were not recorded)\n".into(),
                mv: rec.clone(),
            });
        }
        for color in Color::BOTH {
            let keep = if color == board.turn() { HISTORY_TURNS - 1 } else { HISTORY_TURNS };
            while self.history[color.index()].len() > keep {
                self.history[color.index()].pop_front();
            }
        }
        for color in Color::BOTH {
            let just_moved = color != board.turn() && !self.history[color.index()].is_empty();
            self.write_history(color, just_moved)?;
            if color != board.turn() {
                fs::write(
                    self.future_path(color),
                    format!(
                        "{}\n\nWaiting: {} is not the next player to move in this position.\n\
                         This file fills in after {}'s next move.\n",
                        future_title(color),
                        color.name(),
                        color.other().name()
                    ),
                )?;
            }
        }
        self.start_future_search(board)
    }

    /// Steps 4 and 5: record a confirmed move, then refresh the future-search
    /// file of the player who is now to move.
    pub fn record_turn(&mut self, before: &Board, m: &Move, san: &str, after: &Board) -> io::Result<()> {
        self.record_turn_then(before, m, san, after, |_| ())
    }

    /// The same as [`record_turn`](Self::record_turn), and `between` runs after
    /// the two history files are written and before the future search of the
    /// new position starts. At that moment the four files hold the past, the
    /// present (the move just made) and the finished future search of the
    /// position where the player decided. Step 15 copies them then.
    pub fn record_turn_then<T>(&mut self, before: &Board, m: &Move, san: &str, after: &Board, between: impl FnOnce(&GameFiles) -> T) -> io::Result<T> {
        let mut mv = RecordedMove::without_san(m);
        mv.san = san.to_string();
        let record = TurnRecord {
            move_number: before.fullmove_number(),
            color: m.color,
            san: san.to_string(),
            uci: m.uci(),
            before: moves_listing(before),
            after: moves_listing(after),
            mv,
        };
        // The player who just moved keeps four previous turns and the current
        // one. The other player is not mid-turn, so keeps only its previous four.
        let list = &mut self.history[m.color.index()];
        list.push_back(record);
        while list.len() > HISTORY_TURNS {
            list.pop_front();
        }
        let other = &mut self.history[m.color.other().index()];
        while other.len() > HISTORY_TURNS - 1 {
            other.pop_front();
        }
        self.write_history(m.color, true)?;
        self.write_history(m.color.other(), false)?;
        let result = between(self);
        self.start_future_search(after)?;
        Ok(result)
    }

    pub fn history(&self, color: Color) -> &VecDeque<TurnRecord> {
        &self.history[color.index()]
    }

    fn write_history(&self, color: Color, just_moved: bool) -> io::Result<()> {
        let mut text = format!(
            "{} HISTORY FILE (Steps 4 and 5)\n\
             Keeps {}'s previous four turns and the current turn, oldest first.\n\
             Each turn lists every piece's available legal moves before and after the move was finalized.\n\
             Moves for the player who is not to move are the moves it would have if it were its turn.\n\
             Lines starting with RECORD are read back by the program to build the strategy window.\n",
            color.name().to_uppercase(),
            color.name()
        );
        let list = &self.history[color.index()];
        if list.is_empty() {
            text.push_str(&format!("\nNo {} turns have been played in this game yet.\n", color.name()));
        }
        let n = list.len();
        for (i, rec) in list.iter().enumerate() {
            let label = if just_moved && i + 1 == n {
                "current turn".to_string()
            } else if just_moved {
                format!("previous turn {}", n - 1 - i)
            } else {
                format!("previous turn {}", n - i)
            };
            let number = match rec.color {
                Color::White => format!("{}.", rec.move_number),
                Color::Black => format!("{}...", rec.move_number),
            };
            text.push_str(&format!(
                "\n========================================================================\n\
                 {} {} {} ({}) — {}\n\
                 ========================================================================\n\
                 {}\n\
                 -- Before the move --\n{}\n-- After the move --\n{}",
                rec.color.name(),
                number,
                rec.san,
                rec.uci,
                label,
                record_line(&rec.mv),
                rec.before,
                rec.after
            ));
        }
        fs::write(self.history_path(color), text)
    }

    /// The past: every move recorded in the two history files, read back from disk.
    /// The strategy window (previous four turns, plus the current one when that
    /// player has just moved) is cut from this by the evaluator.
    pub fn history_moves(&self) -> Vec<RecordedMove> {
        let mut out = Vec::new();
        for color in Color::BOTH {
            if let Ok(text) = fs::read_to_string(self.history_path(color)) {
                out.extend(text.lines().filter_map(parse_record_line));
            }
        }
        out
    }

    /// Step 6: rebuild the future-search file of the side to move in the background.
    pub fn start_future_search(&mut self, board: &Board) -> io::Result<()> {
        let color = board.turn();
        if let Some(old) = self.workers[color.index()].take() {
            old.stop();
        }
        let path = self.future_path(color);
        fs::write(
            &path,
            format!(
                "{}\n\nPosition: {}\nStatus: searching all legal sequences to six individual moves…\n",
                future_title(color),
                board.fen()
            ),
        )?;
        let moves = self.history_moves();
        let progress = Arc::new(Progress::default());
        let shared = Arc::clone(&progress);
        let board = *board;
        let handle = std::thread::spawn(move || {
            if let Some(report) = full_search(&board, &moves, &shared) {
                let _ = fs::write(&path, future_text(color, &report));
            }
        });
        self.workers[color.index()] = Some(Worker { progress, handle: Some(handle) });
        Ok(())
    }

    /// The future, for the position `board`: the strategy evidence stored in the
    /// future-search file of the side to move, which Lists 1 to 4 read. Waits for
    /// that search to finish. Returns `None` when the file holds no finished
    /// search of exactly this position. A search is never started from here.
    pub fn stored_search(&mut self, board: &Board) -> Option<FutureStore> {
        let color = board.turn();
        if let Some(w) = self.workers[color.index()].as_mut() {
            if let Some(h) = w.handle.take() {
                let _ = h.join();
            }
        }
        let text = fs::read_to_string(self.future_path(color)).ok()?;
        FutureStore::from_text(&text).filter(|s| s.fen == board.fen())
    }

    /// True while the search for the side to move is still running.
    pub fn search_running_for(&self, color: Color) -> bool {
        self.workers[color.index()].as_ref().is_some_and(|w| !w.finished())
    }

    /// One-line status of the two future searches.
    pub fn search_status(&self) -> String {
        Color::BOTH
            .iter()
            .map(|&c| {
                let state = match &self.workers[c.index()] {
                    None => "waiting".to_string(),
                    Some(w) if w.finished() => "complete".to_string(),
                    Some(w) => {
                        let done = w.progress.roots_done.load(Ordering::Relaxed);
                        let total = w.progress.roots_total.load(Ordering::Relaxed).max(1);
                        format!("searching {}%", done * 100 / total)
                    }
                };
                format!("{} future file: {}", c.name(), state)
            })
            .collect::<Vec<_>>()
            .join(" · ")
    }

    pub fn is_searching(&self) -> bool {
        self.workers.iter().flatten().any(|w| !w.finished())
    }

    /// Block until both future searches have finished.
    pub fn wait_for_searches(&mut self) {
        for w in self.workers.iter_mut().flatten() {
            if let Some(h) = w.handle.take() {
                let _ = h.join();
            }
        }
    }

    pub fn stop_workers(&mut self) {
        for slot in self.workers.iter_mut() {
            if let Some(w) = slot.take() {
                w.stop();
            }
        }
    }
}

impl Drop for GameFiles {
    fn drop(&mut self) {
        self.stop_workers();
    }
}

// ----- RECORD lines (the past) -------------------------------------------------

pub fn record_line(m: &RecordedMove) -> String {
    let letter = |k: Option<PieceKind>| k.map(|k| k.letter().to_string()).unwrap_or_else(|| "-".into());
    format!(
        "RECORD {} {} {} {} {} {} {} {}",
        m.color.name(),
        square_name(m.from),
        square_name(m.to),
        m.piece.letter(),
        letter(m.captured),
        letter(m.promotion),
        if m.en_passant { 1 } else { 0 },
        if m.san.is_empty() { "-" } else { m.san.as_str() }
    )
}

pub fn parse_record_line(line: &str) -> Option<RecordedMove> {
    let rest = line.strip_prefix("RECORD ")?;
    let p: Vec<&str> = rest.split_whitespace().collect();
    if p.len() != 8 {
        return None;
    }
    let kind = |s: &str| -> Option<Option<PieceKind>> {
        if s == "-" {
            Some(None)
        } else {
            PieceKind::from_letter(s.chars().next()?).map(Some)
        }
    };
    Some(RecordedMove {
        color: Color::from_letter(p[0].chars().next()?)?,
        from: parse_square(p[1])?,
        to: parse_square(p[2])?,
        piece: PieceKind::from_letter(p[3].chars().next()?)?,
        captured: kind(p[4])?,
        promotion: kind(p[5])?,
        en_passant: p[6] == "1",
        san: if p[7] == "-" { String::new() } else { p[7].to_string() },
    })
}

// ----- Future file ------------------------------------------------------------------

fn future_title(color: Color) -> String {
    format!(
        "{} FUTURE-SEARCH FILE (Step 6)\n\
         Three full turns from the latest position where {} is the next player to move:\n\
         three individual moves by each player, six in total, every legal sequence examined.\n\
         Lists 1, 3 and 4 read the strategy evidence stored at the end of this file.",
        color.name().to_uppercase(),
        color.name()
    )
}

fn future_text(color: Color, r: &SearchReport) -> String {
    let mut t = future_title(color);
    t.push_str(&format!(
        "\n\nPosition: {}\nStatus: complete in {:.2} s on {} thread(s)\n\nSequences by depth:\n",
        r.fen,
        r.elapsed.as_secs_f64(),
        r.threads
    ));
    for d in 0..SEARCH_PLIES {
        t.push_str(&format!(
            "  Individual move {} ({}): {:>15} possible sequences\n",
            d + 1,
            mover_at(r.to_move, d).name(),
            thousands(r.counts[d])
        ));
    }
    t.push_str(&format!("  Total search nodes: {}\n", thousands(r.total_nodes())));

    t.push_str("\nSequences by first move (counts at individual moves 1–6):\n");
    for root in &r.roots {
        let cells: Vec<String> = root.counts.iter().map(|&c| thousands(c)).collect();
        t.push_str(&format!("  {:<8} {}\n", root.san, cells.join(" / ")));
    }
    if r.roots.is_empty() {
        t.push_str("  (no legal moves: the game is over in this position)\n");
    }

    t.push_str(&format!(
        "\nEvery sequence of up to {LISTED_PLIES} individual moves ({} lines).\n\
         Deeper sequences are counted above but not listed one per line: at six moves that\n\
         would be {} lines here.\n\n",
        thousands(r.lines.len() as u64),
        thousands(r.counts[SEARCH_PLIES - 1])
    ));
    for line in &r.lines {
        t.push_str(line);
        t.push('\n');
    }

    t.push_str(
        "\n\nSTORED STRATEGY EVIDENCE (read by List 1, List 3 and List 4)\n\
         One line for the searched position, for each position after one move, and for each position\n\
         after a move and a reply (UCI moves). Each line holds four rows of 20 digits, one digit per\n\
         strategy 1-20: White progress-or-completion, White completion, Black progress-or-completion,\n\
         Black completion. A digit is the earliest individual move, counted from that\n\
         position, at which it appears on some legal line of the six-move search. 0 means never.\n\
         The development score reads digits 1 and 2 (one full turn). The rest is the horizon.\n",
    );
    for line in r.store.to_lines() {
        t.push_str(&line);
        t.push('\n');
    }
    t
}

/// Every piece on the board with its available legal moves (Step 4).
pub fn moves_listing(board: &Board) -> String {
    let mut out = String::new();
    let to_move = board.turn();
    for color in [to_move, to_move.other()] {
        let shown = board.with_turn(color);
        let moves = shown.legal_moves();
        let label = if color == to_move {
            format!("{} pieces ({} to move):", color.name(), color.name())
        } else {
            format!("{} pieces (moves if it were {}'s turn):", color.name(), color.name())
        };
        out.push_str(&label);
        out.push('\n');
        let mut pieces: Vec<(PieceKind, u8)> = board
            .pieces_in_board_order()
            .into_iter()
            .filter(|(_, p)| p.color == color)
            .map(|(sq, p)| (p.kind, sq))
            .collect();
        pieces.sort_by_key(|&(k, sq)| (std::cmp::Reverse(k), square_name(sq)));
        for (kind, sq) in pieces {
            let mine: Vec<String> = moves.iter().filter(|m| m.from == sq).map(|m| shown.san_with(m, &moves)).collect();
            let list = if mine.is_empty() { "(no legal moves)".to_string() } else { mine.join(" ") };
            out.push_str(&format!("  {:<6} {}: {}\n", capitalize(kind.name()), square_name(sq), list));
        }
    }
    out
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}
