//! Step 15: keep the files made by Steps 4, 5 and 6.
//!
//! When the player answers Y at the start of a game, every confirmed move
//! appends one record to that game's single archive file,
//! `ai_learning/game_<id>.txt`, while the four working files still hold it:
//!
//! * the two history files (Steps 4 and 5) and the two future-search files
//!   (Step 6), copied whole and unchanged, byte for byte. Together they are the
//!   move's past (the four past turns of each side), its present (the turn just
//!   played, in the history file of the side that moved) and its future (the
//!   finished six-move search of the position where the player decided);
//! * the official List 1 for both sides, List 2, List 3 for every legal
//!   candidate (the played one marked PLAYED, every line labelled `searched`)
//!   and List 4 for every reply, as Steps 8–13 made them from those files.
//!
//! A file is never rewritten, only appended to. It ends with `GAME END` (a
//! result) or `GAME STOPPED`. A game left open because the program was killed
//! gets `GAME STOPPED` at the next start. Finished games enter the memory list
//! (`ai_learning/memory_index.txt`) and the general memory bank
//! (`ai_learning/banks/general.txt`). Saving these copies is not training.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::board::{square_name, Board, Color, Move};
use crate::evaluate::{
    evaluate_stored, list3_all_stored, list4_all_stored, CaptureRow, Evaluation, StrategyHit,
};
use crate::files::{record_line, GameFiles};
use crate::geometry::Unit;
use crate::position::RecordedMove;
use crate::search::{FutureStore, SEARCH_PLIES};
use crate::strategies::STRATEGIES;

/// Archive file layout. 1 = the first archive (MEMORY records), 3 = Step 15.
pub const FORMAT: u32 = 3;
pub const ARCHIVE_FOLDER: &str = "ai_learning";
pub const MEMORY_INDEX: &str = "memory_index.txt";
pub const BANKS_FOLDER: &str = "banks";
pub const GENERAL_BANK: &str = "general.txt";
/// The most memories the reader returns when a caller asks for a limited set (15.8).
pub const MEMORY_LIMIT: usize = 50;

/// The question asked at startup and on new, load and preset (15.3).
pub const RECORD_QUESTION: &str =
    "Save this game's information to the permanent AI learning files?\nY — Yes\nN — No";

pub fn archive_dir(game_dir: &Path) -> PathBuf {
    game_dir.join(ARCHIVE_FOLDER)
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ----- Outcome -------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// "1-0", "0-1" or "1/2-1/2".
    pub code: String,
    /// checkmate, resignation, stalemate, draw agreement, insufficient material,
    /// fifty-move rule or threefold repetition.
    pub termination: String,
    /// The sentence shown to the player.
    pub text: String,
}

impl Outcome {
    pub fn win(winner: Color, termination: &str, text: String) -> Outcome {
        let code = if winner == Color::White { "1-0" } else { "0-1" };
        Outcome {
            code: code.into(),
            termination: termination.into(),
            text,
        }
    }
    pub fn draw(termination: &str, text: String) -> Outcome {
        Outcome {
            code: "1/2-1/2".into(),
            termination: termination.into(),
            text,
        }
    }
}

// ----- The four working files at one moment --------------------------------------

/// The four working files, byte for byte, at the moment a move was recorded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkingFiles {
    /// (file name, exact bytes), in the order white history, white future,
    /// black history, black future.
    pub files: Vec<(String, Vec<u8>)>,
}

impl WorkingFiles {
    /// Read the four files as they are on disk now.
    pub fn read(files: &GameFiles) -> io::Result<WorkingFiles> {
        let mut out = Vec::new();
        for path in files.all_paths() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?")
                .to_string();
            out.push((name, fs::read(&path)?));
        }
        Ok(WorkingFiles { files: out })
    }

    pub fn get(&self, name: &str) -> Option<&[u8]> {
        self.files
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, b)| b.as_slice())
    }
}

pub fn history_name(c: Color) -> String {
    format!("{}_history.txt", c.name().to_lowercase())
}

pub fn future_name(c: Color) -> String {
    format!("{}_future.txt", c.name().to_lowercase())
}

// ----- Writing --------------------------------------------------------------------

/// Every archive file in the folder, by game id.
pub fn archive_files(dir: &Path) -> Vec<(u32, PathBuf)> {
    let mut out: Vec<(u32, PathBuf)> = fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter_map(|p| game_id_of(&p).map(|id| (id, p)))
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

fn game_id_of(path: &Path) -> Option<u32> {
    let name = path.file_name()?.to_str()?;
    name.strip_prefix("game_")?
        .strip_suffix(".txt")?
        .parse()
        .ok()
}

/// The archive file of one recorded game. Append-only.
pub struct GameArchive {
    id: u32,
    dir: PathBuf,
    path: PathBuf,
    file: File,
    plies: u32,
    closed: bool,
}

impl GameArchive {
    /// Start a new game's file with the next unused number. Never opens an existing file.
    pub fn create(dir: &Path, board: &Board, seed: &[RecordedMove]) -> io::Result<GameArchive> {
        fs::create_dir_all(dir)?;
        let mut id = archive_files(dir).last().map(|(id, _)| id + 1).unwrap_or(1);
        let (path, file) = loop {
            let path = dir.join(format!("game_{id}.txt"));
            match OpenOptions::new().append(true).create_new(true).open(&path) {
                Ok(f) => break (path, f),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => id += 1,
                Err(e) => return Err(e),
            }
        };
        let mut a = GameArchive {
            id,
            dir: dir.to_path_buf(),
            path,
            file,
            plies: 0,
            closed: false,
        };
        let mut t = String::from("GAME START\n");
        t.push_str(&format!("id: {id}\n"));
        t.push_str(&format!("format: {FORMAT}\n"));
        t.push_str(&format!("engine: chess {}\n", env!("CARGO_PKG_VERSION")));
        t.push_str(&format!("search_plies: {SEARCH_PLIES}\n"));
        t.push_str(&format!("fen: {}\n", board.fen()));
        for rec in seed {
            t.push_str(&format!("seed: {}\n", record_line(rec)));
        }
        t.push_str(&format!("time_unix: {}\n", now_unix()));
        t.push_str("END GAME START\n");
        a.append(t.as_bytes())?;
        Ok(a)
    }

    pub fn id(&self) -> u32 {
        self.id
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn plies(&self) -> u32 {
        self.plies
    }
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    fn append(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.file.write_all(bytes)?;
        self.file.flush()?;
        self.file.sync_data()
    }

    /// Append the record of one confirmed move (15.5).
    ///
    /// * `before` / `after` – the positions around the move;
    /// * `lists_past` – the RECORD lines the lists read while the player decided;
    /// * `files` – the four working files at the moment the move was recorded:
    ///   the history files already hold the move as the current turn, and the
    ///   future-search file of the side that moved is the finished six-move
    ///   search of the position where it decided.
    pub fn record_move(
        &mut self,
        before: &Board,
        m: &Move,
        san: &str,
        after: &Board,
        lists_past: &[RecordedMove],
        files: &WorkingFiles,
    ) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }
        let bytes = move_record(
            self.id,
            self.plies + 1,
            before,
            m,
            san,
            after,
            lists_past,
            files,
        );
        self.append(&bytes)?;
        self.plies += 1;
        Ok(())
    }

    /// The game has a result. Its moves enter the memory list and the general bank.
    pub fn finish(&mut self, outcome: &Outcome) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }
        let t = format!(
            "GAME END\nresult_code: {}\ntermination: {}\nresult: {}\nplies: {}\ntime_unix: {}\nEND GAME END\n",
            outcome.code,
            outcome.termination,
            outcome.text,
            self.plies,
            now_unix()
        );
        self.append(t.as_bytes())?;
        self.closed = true;
        add_to_memory(&self.dir, &self.path)
    }

    /// The game stopped without a result. Its moves enter the memory list and the general bank.
    pub fn stop(&mut self, reason: &str) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }
        let t = stop_block(reason, self.plies);
        self.append(t.as_bytes())?;
        self.closed = true;
        add_to_memory(&self.dir, &self.path)
    }
}

fn stop_block(reason: &str, plies: u32) -> String {
    format!("GAME STOPPED\nresult_code: *\nreason: {reason}\nplies: {plies}\ntime_unix: {}\nEND GAME STOPPED\n", now_unix())
}

fn file_section(out: &mut Vec<u8>, name: &str, note: &str, bytes: &[u8]) {
    out.extend_from_slice(format!("FILE {name} bytes={}\n# {note}\n", bytes.len()).as_bytes());
    out.extend_from_slice(bytes);
    out.extend_from_slice(b"\nEND FILE\n");
}

#[allow(clippy::too_many_arguments)]
fn move_record(
    game: u32,
    ply: u32,
    before: &Board,
    m: &Move,
    san: &str,
    after: &Board,
    lists_past: &[RecordedMove],
    files: &WorkingFiles,
) -> Vec<u8> {
    let side = m.color;
    let mut rec = RecordedMove::new(before, m);
    rec.san = san.to_string();
    let mut head = format!("MOVE game={game} ply={ply}\n");
    head.push_str(&format!("move_number: {}\n", before.fullmove_number()));
    head.push_str(&format!("side: {}\n", side.name()));
    head.push_str("mover: human\n");
    head.push_str(&format!("record: {}\n", record_line(&rec)));
    head.push_str(&format!("uci: {}\n", m.uci()));
    head.push_str(&format!("fen_before: {}\n", before.fen()));
    head.push_str(&format!("fen_after: {}\n", after.fen()));
    head.push_str(
        "# The four working files follow, copied whole and unchanged at the moment this move was recorded.\n\
         # Past: the previous turns in both history files. Present: the current turn in the history file\n\
         # of the side that moved. Future: the finished six-move future-search file of the side that moved,\n\
         # searched from the position where it decided (fen_before).\n",
    );
    let mut out = head.into_bytes();
    for c in Color::BOTH {
        let h = history_name(c);
        let note = if c == side {
            format!(
                "{} history file: past turns and the present turn (this move)",
                c.name()
            )
        } else {
            format!("{} history file: past turns", c.name())
        };
        file_section(&mut out, &h, &note, files.get(&h).unwrap_or(b""));
        let f = future_name(c);
        let note = if c == side {
            format!("{} future-search file: the FUTURE of this move, the finished six-move search of fen_before", c.name())
        } else {
            format!(
                "{} future-search file as it stood (the search of {}'s own last decision)",
                c.name(),
                c.name()
            )
        };
        file_section(&mut out, &f, &note, files.get(&f).unwrap_or(b""));
    }
    let future = files
        .get(&future_name(side))
        .and_then(|b| std::str::from_utf8(b).ok())
        .and_then(FutureStore::from_text);
    out.extend_from_slice(lists_block(before, side, m, lists_past, future.as_ref()).as_bytes());
    out.extend_from_slice(b"END MOVE\n");
    out
}

// ----- Lists 1–4 ------------------------------------------------------------------

fn units(us: &[Unit]) -> String {
    if us.is_empty() {
        return "-".into();
    }
    us.iter()
        .map(|u| {
            format!(
                "{}{}{}",
                u.color.letter(),
                u.kind.letter(),
                square_name(u.square)
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn strategy_number(id: &str) -> usize {
    STRATEGIES
        .iter()
        .position(|d| d.id == id)
        .map(|i| i + 1)
        .unwrap_or(0)
}

fn opt(n: Option<u8>) -> String {
    n.map(|n| n.to_string()).unwrap_or_else(|| "-".into())
}

fn clean(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

fn hit_line(list: u8, context: &str, side: Color, rank: usize, h: &StrategyHit) -> String {
    format!(
        "LIST{list}\t{context}\t{}\t{rank}\t{}\t{}\t{}\t{}\tfoundation={} progress={} completion={}\tprogress_within_turn={} completion_within_turn={}\tprogress_by_move={}\tcompletion_by_move={}\t{}\t{}\tparticipants={}\ttargets={}\n",
        side.name(),
        strategy_number(h.id),
        h.name,
        h.score,
        h.stage.label(),
        h.foundation as u8,
        h.progress as u8,
        h.completion as u8,
        h.progress_within_turn as u8,
        h.completion_within_turn as u8,
        opt(h.progress_in),
        opt(h.completion_in),
        clean(h.explanation),
        clean(&h.evidence),
        units(&h.participants),
        units(&h.targets)
    )
}

fn capture_line(list: u8, context: &str, side: Color, rank: usize, c: &CaptureRow) -> String {
    format!(
        "LIST{list}\t{context}\t{}\t{rank}\t{}\t{} {} takes {} {}\tgain={}\tloss={}\tnet={}\tmate={}\trecapture={}\t{}\t{}\n",
        side.name(),
        c.san,
        c.capturer.name(),
        square_name(c.from),
        c.victim.name(),
        square_name(c.victim_square),
        c.expected_gain,
        c.potential_loss,
        c.net,
        c.mate as u8,
        if c.recapture_sans.is_empty() { "-".to_string() } else { c.recapture_sans.join(" ") },
        strategy_number(c.strategy_id),
        c.strategy_name
    )
}

fn push_list(out: &mut String, list: u8, context: &str, e: &Evaluation) {
    for (i, h) in e.hits.iter().enumerate() {
        out.push_str(&hit_line(list, context, e.side, i + 1, h));
    }
}

fn push_captures(out: &mut String, list: u8, context: &str, e: &Evaluation) {
    for (i, c) in e.captures.iter().enumerate() {
        out.push_str(&capture_line(list, context, e.side, i + 1, c));
    }
}

/// The official lists for the position where the player decided, made by
/// Steps 8–13 from the saved future-search file and the history the player had.
pub fn lists_block(
    before: &Board,
    side: Color,
    played: &Move,
    past: &[RecordedMove],
    store: Option<&FutureStore>,
) -> String {
    let mut t = String::from("LISTS\n");
    for p in past {
        t.push_str(&format!("lists_past: {}\n", record_line(p)));
    }
    let Some(store) = store.filter(|s| s.fen == before.fen()) else {
        t.push_str("lists: unavailable (the future-search file of this position was not finished)\nEND LISTS\n");
        return t;
    };
    t.push_str(
        "# LISTn <context> <side> <rank> <strategy#> <strategy> <score> <stage> <now> <within one full turn> <earliest progress> <earliest completion> <why the score> <evidence> <participants> <targets>\n\
         # LIST2/LIST4 <context> <side> <rank> <move> <capture> <gain> <loss> <net> <mate> <recapture> <strategy#> <strategy>\n\
         # context: current for Lists 1 and 2; 'searched <move>' for List 3; 'searched <move> <reply>' for List 4.\n\
         # A searched line is a possible future, not a move that was played. The played candidate is marked PLAYED.\n",
    );
    for s in [side, side.other()] {
        let e = evaluate_stored(before, s, past, store, &[]);
        push_list(&mut t, 1, "current", &e);
        if s == side {
            push_captures(&mut t, 2, "current", &e);
        }
    }
    for branch in list3_all_stored(before, side, past, store) {
        let uci = branch.mv.uci();
        let mark = if branch.mv.uci() == played.uci() {
            "PLAYED"
        } else {
            "-"
        };
        t.push_str(&format!("CAND\tsearched\t{uci}\t{}\t{mark}\n", branch.san));
        let ctx = format!("searched {uci}");
        push_list(&mut t, 3, &ctx, &branch.evaluation);
        for (reply_san, e) in list4_all_stored(&branch, store) {
            if e.captures.is_empty() {
                continue;
            }
            let reply = branch
                .board
                .parse_san(&reply_san)
                .map(|r| r.uci())
                .unwrap_or(reply_san);
            push_captures(&mut t, 4, &format!("searched {uci} {reply}"), &e);
        }
    }
    t.push_str("END LISTS\n");
    t
}

// ----- Reading --------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GameStatus {
    Finished {
        result_code: String,
        termination: String,
        text: String,
    },
    Stopped {
        reason: String,
    },
    /// No end block yet (the game is being recorded, or the program was killed
    /// and has not been started again).
    Open,
}

/// One saved move.
#[derive(Clone, Debug)]
pub struct SavedMove {
    pub ply: u32,
    pub side: Color,
    pub san: String,
    pub uci: String,
    pub fen_before: String,
    pub fen_after: String,
    /// The copied working files, byte for byte (format 3).
    pub files: WorkingFiles,
    /// The LIST and CAND lines.
    pub lists: Vec<String>,
}

impl SavedMove {
    pub fn file(&self, name: &str) -> Option<&[u8]> {
        self.files.get(name)
    }
    /// The finished six-move search the player decided with, as the lists read it.
    pub fn future_store(&self) -> Option<FutureStore> {
        let b = self.files.get(&future_name(self.side))?;
        FutureStore::from_text(std::str::from_utf8(b).ok()?)
    }
}

#[derive(Clone, Debug)]
pub struct ArchivedGame {
    pub id: u32,
    pub path: PathBuf,
    pub format: u32,
    pub start_fen: Option<String>,
    pub moves: Vec<SavedMove>,
    pub status: GameStatus,
}

struct Block {
    first: String,
    lines: Vec<String>,
    files: Vec<(String, Vec<u8>)>,
}

/// Read the next line (without its newline) from `pos`.
fn next_line(b: &[u8], pos: &mut usize) -> Option<String> {
    if *pos >= b.len() {
        return None;
    }
    let end = b[*pos..]
        .iter()
        .position(|&c| c == b'\n')
        .map(|i| *pos + i)
        .unwrap_or(b.len());
    let line = String::from_utf8_lossy(&b[*pos..end])
        .trim_end_matches('\r')
        .to_string();
    *pos = (end + 1).min(b.len() + 1);
    Some(line)
}

fn end_of(first: &str) -> Option<String> {
    match first.split_whitespace().next()? {
        "GAME" => Some(format!("END {}", first.trim())),
        "MOVE" => Some("END MOVE".into()),
        "MEMORY" => Some("END MEMORY".into()),
        _ => None,
    }
}

/// Split a file into its complete blocks. A record cut off when the program
/// was killed is skipped up to the GAME STOPPED that the next start appended.
fn blocks(b: &[u8]) -> Vec<Block> {
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(first) = next_line(b, &mut pos) {
        let Some(end) = end_of(&first) else { continue };
        let start = pos;
        let mut block = Block {
            first: first.clone(),
            lines: Vec::new(),
            files: Vec::new(),
        };
        let mut complete = false;
        while let Some(line) = next_line(b, &mut pos) {
            if line == end {
                complete = true;
                break;
            }
            if let Some(rest) = line.strip_prefix("FILE ") {
                let mut parts = rest.split_whitespace();
                let name = parts.next().unwrap_or("").to_string();
                let Some(n) = parts
                    .next()
                    .and_then(|p| p.strip_prefix("bytes="))
                    .and_then(|n| n.parse::<usize>().ok())
                else {
                    break;
                };
                let Some(note) = next_line(b, &mut pos) else {
                    break;
                };
                if !note.starts_with('#') || pos + n > b.len() {
                    break;
                }
                let content = b[pos..pos + n].to_vec();
                pos += n;
                if b.get(pos..pos + 10) != Some(b"\nEND FILE\n".as_slice()) {
                    break;
                }
                pos += 10;
                block.files.push((name, content));
                continue;
            }
            block.lines.push(line);
        }
        if complete {
            out.push(block);
        } else {
            // A cut-off record: resume at the next GAME STOPPED after it, if any.
            match find(b, b"\nGAME STOPPED\n", start) {
                Some(i) => pos = i + 1,
                None => break,
            }
        }
    }
    out
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from >= hay.len() {
        return None;
    }
    hay[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|i| from + i)
}

fn field<'a>(lines: &'a [String], key: &str) -> Option<&'a str> {
    lines
        .iter()
        .find_map(|l| l.strip_prefix(key))
        .map(|v| v.trim())
}

fn number_after(first: &str, key: &str) -> Option<u32> {
    first
        .split_whitespace()
        .find_map(|w| w.strip_prefix(key))
        .and_then(|n| n.parse().ok())
}

fn legacy_code(text: &str) -> String {
    if text.contains("White wins") {
        "1-0".into()
    } else if text.contains("Black wins") {
        "0-1".into()
    } else if text.to_lowercase().contains("draw") {
        "1/2-1/2".into()
    } else {
        "*".into()
    }
}

/// Read one archive file (format 1 or 3).
pub fn read_game(path: &Path) -> io::Result<ArchivedGame> {
    let bytes = fs::read(path)?;
    let mut g = ArchivedGame {
        id: game_id_of(path).unwrap_or(0),
        path: path.to_path_buf(),
        format: 1,
        start_fen: None,
        moves: Vec::new(),
        status: GameStatus::Open,
    };
    for b in blocks(&bytes) {
        let l = &b.lines;
        match b.first.as_str() {
            "GAME START" => {
                g.format = field(l, "format:")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1);
                g.start_fen = field(l, "fen:").map(String::from);
            }
            "GAME END" => {
                let text = field(l, "result:").unwrap_or("").to_string();
                g.status = GameStatus::Finished {
                    result_code: field(l, "result_code:")
                        .map(String::from)
                        .unwrap_or_else(|| legacy_code(&text)),
                    termination: field(l, "termination:").unwrap_or("").to_string(),
                    text,
                };
            }
            "GAME STOPPED" => {
                if g.status == GameStatus::Open {
                    g.status = GameStatus::Stopped {
                        reason: field(l, "reason:").unwrap_or("").to_string(),
                    };
                }
            }
            f if f.starts_with("MOVE ") || f.starts_with("MEMORY ") => {
                let record = field(l, "record:").and_then(crate::files::parse_record_line);
                g.moves.push(SavedMove {
                    ply: number_after(f, "ply=").unwrap_or(0),
                    side: field(l, "side:")
                        .and_then(|s| s.chars().next())
                        .and_then(Color::from_letter)
                        .unwrap_or(Color::White),
                    san: record.as_ref().map(|r| r.san.clone()).unwrap_or_default(),
                    uci: field(l, "uci:")
                        .map(String::from)
                        .or_else(|| {
                            record
                                .as_ref()
                                .map(|r| format!("{}{}", square_name(r.from), square_name(r.to)))
                        })
                        .unwrap_or_default(),
                    fen_before: field(l, "fen_before:").unwrap_or("").to_string(),
                    fen_after: field(l, "fen_after:").unwrap_or("").to_string(),
                    files: WorkingFiles { files: b.files },
                    lists: l
                        .iter()
                        .filter(|x| x.starts_with("LIST") || x.starts_with("CAND"))
                        .cloned()
                        .collect(),
                });
            }
            _ => {}
        }
    }
    Ok(g)
}

/// Every archive file in the folder, read, by id.
pub fn read_archive(dir: &Path) -> Vec<ArchivedGame> {
    archive_files(dir)
        .into_iter()
        .filter_map(|(_, p)| read_game(&p).ok())
        .collect()
}

/// A game left open because the program was killed gets GAME STOPPED, appended
/// after its last byte; nothing earlier changes. The memory list and the general
/// bank are then rebuilt. Returns the ids of the games closed this way.
pub fn recover(dir: &Path) -> io::Result<Vec<u32>> {
    let mut closed = Vec::new();
    for (id, path) in archive_files(dir) {
        let bytes = fs::read(&path)?;
        if !blocks(&bytes)
            .iter()
            .any(|b| b.first == "GAME END" || b.first == "GAME STOPPED")
        {
            let plies = blocks(&bytes)
                .iter()
                .filter(|b| b.first.starts_with("MOVE "))
                .count() as u32;
            let mut f = OpenOptions::new().append(true).open(&path)?;
            let mut t = String::new();
            if !bytes.ends_with(b"\n") {
                t.push('\n');
            }
            t.push_str(&stop_block(
                "the program was stopped before the game ended (found at the next start)",
                plies,
            ));
            f.write_all(t.as_bytes())?;
            f.sync_data()?;
            closed.push(id);
        }
    }
    // Rebuild when a game was closed here, or when the list or the general bank
    // is missing or was written by an earlier version in another layout.
    let ours =
        |path: PathBuf, header: &str| fs::read_to_string(path).is_ok_and(|t| t.starts_with(header));
    let current = ours(dir.join(MEMORY_INDEX), INDEX_HEADER)
        && ours(dir.join(BANKS_FOLDER).join(GENERAL_BANK), BANK_HEADER);
    if !closed.is_empty() || (!archive_files(dir).is_empty() && !current) {
        rebuild_memory(dir)?;
    }
    Ok(closed)
}

// ----- Memory list and banks (15.8, 15.9) ------------------------------------------

/// One saved move in the memory list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Memory {
    pub game: u32,
    pub ply: u32,
}

fn memory_lines(g: &ArchivedGame) -> Vec<String> {
    let end = match &g.status {
        GameStatus::Finished {
            result_code,
            termination,
            ..
        } => format!("end=GAME END {result_code} {termination}"),
        GameStatus::Stopped { .. } => "end=GAME STOPPED *".into(),
        GameStatus::Open => return vec![],
    };
    g.moves
        .iter()
        .map(|m| {
            format!(
                "game={} ply={}\tside={}\tmove={}\tformat={}\t{}",
                g.id,
                m.ply,
                m.side.name(),
                m.san,
                g.format,
                end.trim_end()
            )
        })
        .collect()
}

const INDEX_HEADER: &str = "# Memory list (Step 15.8): one line per saved move, by game id and ply, in recording order.\n\
# A game is listed only after its GAME END or GAME STOPPED block. This file can be deleted and rebuilt from the archive files.\n";
const BANK_HEADER: &str = "# General memory bank (Step 15.9): references to every saved move of every game that has an end block.\n";

fn append_lines(path: &Path, header: &str, lines: &[String]) -> io::Result<()> {
    let new = !path.exists();
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    if new {
        f.write_all(header.as_bytes())?;
    }
    for l in lines {
        f.write_all(l.as_bytes())?;
        f.write_all(b"\n")?;
    }
    f.sync_data()
}

fn add_to_memory(dir: &Path, game_path: &Path) -> io::Result<()> {
    let g = read_game(game_path)?;
    let lines = memory_lines(&g);
    append_lines(&dir.join(MEMORY_INDEX), INDEX_HEADER, &lines)?;
    fs::create_dir_all(dir.join(BANKS_FOLDER))?;
    let refs: Vec<String> = lines
        .iter()
        .map(|l| l.split('\t').next().unwrap_or("").to_string())
        .collect();
    append_lines(
        &dir.join(BANKS_FOLDER).join(GENERAL_BANK),
        BANK_HEADER,
        &refs,
    )
}

/// Rewrite the memory list and the general bank from the archive files. The
/// archive files are only read.
pub fn rebuild_memory(dir: &Path) -> io::Result<()> {
    let mut index = String::from(INDEX_HEADER);
    let mut bank = String::from(BANK_HEADER);
    for g in read_archive(dir) {
        for l in memory_lines(&g) {
            bank.push_str(l.split('\t').next().unwrap_or(""));
            bank.push('\n');
            index.push_str(&l);
            index.push('\n');
        }
    }
    fs::write(dir.join(MEMORY_INDEX), index)?;
    fs::create_dir_all(dir.join(BANKS_FOLDER))?;
    fs::write(dir.join(BANKS_FOLDER).join(GENERAL_BANK), bank)
}

fn parse_ref(line: &str) -> Option<Memory> {
    let head = line.split('\t').next()?;
    let mut parts = head.split_whitespace();
    let game = parts.next()?.strip_prefix("game=")?.parse().ok()?;
    let ply = parts.next()?.strip_prefix("ply=")?.parse().ok()?;
    Some(Memory { game, ply })
}

/// The memory list in its stored order. With `limited`, at most
/// [`MEMORY_LIMIT`] memories are returned; nothing is ever deleted.
pub fn memories(dir: &Path, limited: bool) -> Vec<Memory> {
    let text = fs::read_to_string(dir.join(MEMORY_INDEX)).unwrap_or_default();
    let all = text
        .lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(parse_ref);
    if limited {
        all.take(MEMORY_LIMIT).collect()
    } else {
        all.collect()
    }
}

/// The references in a memory bank (`general` for the general bank).
pub fn bank(dir: &Path, name: &str) -> Vec<Memory> {
    let text =
        fs::read_to_string(dir.join(BANKS_FOLDER).join(format!("{name}.txt"))).unwrap_or_default();
    text.lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(parse_ref)
        .collect()
}
