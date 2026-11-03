//! The three-full-turn future search (Step 6).
//!
//! Every legal move sequence is walked to a depth of six individual moves
//! (three by each player). Nothing is pruned or skipped. The search runs on
//! all CPU cores and can be cancelled when the position it describes is
//! superseded.
//!
//! The search is also the future that Lists 1, 3 and 4 read. While it walks,
//! it reads the 20 strategies, for both players, on every position it reaches
//! (individual moves 1 to [`READ_PLIES`]). For each strategy it keeps the
//! *earliest* ply at which progress and at which completion appear on any legal
//! line. That is folded up the tree, so the stored entries answer, for the
//! searched position (List 1), for every position after one move (List 3) and
//! for every position after a move and a reply (List 4):
//!
//! * does progress / completion occur within one full turn (the development
//!   score, Step 14.2), and
//! * how many moves ahead, anywhere in the rest of the six-move tree, do
//!   progress and completion first appear (the horizon).
//!
//! The lists read this stored evidence from the future-search file. They never
//! start a search of their own, and the search is never extended past six moves.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::board::{Board, Color};
use crate::evaluate::FutureFlags;
use crate::position::{EvalContext, RecordedMove};
use crate::strategies::STRATEGIES;

/// Search depth required by Step 6: three moves by White and three by Black.
pub const SEARCH_PLIES: usize = 6;

/// Strategies are read on every position from individual move 1 to this one:
/// all six individual moves of the search (Step 6, Step 15.1).
///
/// Two things keep this fast without changing any result:
/// * a strategy is read on a position only when the reading can still change a
///   stored result (a stored position's earliest progress or completion of that
///   strategy is not yet known at that depth or earlier); a skipped reading could
///   only have offered a later number, which the stored minimum ignores;
/// * the 19 strategies that look only at the board are read once per distinct
///   position and player and reused when the same position is reached by
///   another move order. Central pawn break, which reads the history window, is
///   read on every position that needs it.
///
/// `tests/search_exact.rs` checks the stored evidence against
/// [`full_search_reference`], which reads every strategy on every position.
pub const READ_PLIES: usize = SEARCH_PLIES;

/// Sequences up to this length are written out one per line.
pub const LISTED_PLIES: usize = 3;

/// Number of strategies.
pub const N: usize = 20;

/// Shared progress and cancellation flag for a running search.
#[derive(Default)]
pub struct Progress {
    pub cancel: AtomicBool,
    pub roots_done: AtomicUsize,
    pub roots_total: AtomicUsize,
}

impl Progress {
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

/// For one player and one strategy: the earliest ply (1..=6) below a position at
/// which the strategy shows progress (or completion), and at which it shows
/// completion. 0 means it never does within the search.
pub type Firsts = [[u8; N]; 2];

/// Earliest-occurrence evidence for both players: `first[color][kind][strategy]`,
/// where kind 0 is "progress or completion" and kind 1 is "completion".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Evidence {
    pub first: [Firsts; 2],
}

impl Default for Evidence {
    fn default() -> Self {
        Evidence { first: [[[0; N]; 2]; 2] }
    }
}

fn min_nonzero(a: u8, b: u8) -> u8 {
    match (a, b) {
        (0, x) | (x, 0) => x,
        (x, y) => x.min(y),
    }
}

impl Evidence {
    /// Fold a child's evidence (and the child's own reading) into this node.
    fn absorb(&mut self, reading: &Reading, child: Option<&Evidence>) {
        for c in 0..2 {
            for i in 0..N {
                let bit = 1u32 << i;
                for k in 0..2 {
                    let mask = if k == 0 { reading.progress[c] } else { reading.completion[c] };
                    let mut cand = if mask & bit != 0 { 1 } else { 0 };
                    if cand == 0 {
                        if let Some(ch) = child {
                            let f = ch.first[c][k][i];
                            if f > 0 {
                                cand = f + 1;
                            }
                        }
                    }
                    self.first[c][k][i] = min_nonzero(self.first[c][k][i], cand);
                }
            }
        }
    }

    /// Progress / completion "within one full turn": on some line, within the
    /// next two individual moves (Step 14.1.7).
    pub fn flags(&self, color: Color) -> Vec<FutureFlags> {
        let f = &self.first[color.index()];
        (0..N)
            .map(|i| FutureFlags { progress: (1..=2).contains(&f[0][i]), completion: (1..=2).contains(&f[1][i]) })
            .collect()
    }

    /// For each strategy: the first ply at which progress appears, and at which
    /// completion appears, anywhere in the rest of the stored search (0 = never).
    pub fn horizon(&self, color: Color) -> Vec<(u8, u8)> {
        let f = &self.first[color.index()];
        (0..N).map(|i| (f[0][i], f[1][i])).collect()
    }
}

/// The strategy evidence stored by one search, keyed by the UCI move sequence
/// from the searched position (empty key = the searched position itself).
#[derive(Clone, Debug, Default)]
pub struct FutureStore {
    pub fen: String,
    pub entries: HashMap<String, Evidence>,
}

impl FutureStore {
    pub fn key(path: &[String]) -> String {
        path.join(" ")
    }

    pub fn get(&self, path: &[String]) -> Option<&Evidence> {
        self.entries.get(&Self::key(path))
    }

    /// The text lines written into the future file (one per stored position).
    pub fn to_lines(&self) -> Vec<String> {
        let mut keys: Vec<&String> = self.entries.keys().collect();
        keys.sort_by(|a, b| {
            let n = |s: &String| if s.is_empty() { 0 } else { s.split(' ').count() };
            n(a).cmp(&n(b)).then(a.cmp(b))
        });
        let digits = |a: &[u8; N]| a.iter().map(|d| (b'0' + d) as char).collect::<String>();
        keys.into_iter()
            .map(|k| {
                let e = &self.entries[k];
                format!(
                    "E {} W {} {} B {} {}",
                    if k.is_empty() { "-" } else { k.as_str() },
                    digits(&e.first[0][0]),
                    digits(&e.first[0][1]),
                    digits(&e.first[1][0]),
                    digits(&e.first[1][1])
                )
            })
            .collect()
    }

    /// Read the evidence back from the lines of a future file.
    pub fn from_text(text: &str) -> Option<FutureStore> {
        Self::from_bytes(text.as_bytes())
    }

    /// Read the evidence back from the bytes of a future file.
    ///
    /// Same result as [`from_text`](Self::from_text). The sequence listing
    /// between `Every sequence of up to` and `STORED STRATEGY EVIDENCE` is not
    /// walked line by line when that span contains no `E ` line. That listing
    /// is the bulk of a future file, and the writer never puts an evidence line
    /// in it. A file that does is scanned in full, so a hand-written file still
    /// parses the way it used to.
    pub fn from_bytes(text: &[u8]) -> Option<FutureStore> {
        let (head, tail) = evidence_regions(text);
        let mut fen = None;
        let mut complete = false;
        let mut entries = HashMap::new();
        parse_evidence_lines(head, &mut fen, &mut complete, &mut entries)?;
        if !tail.is_empty() {
            parse_evidence_lines(tail, &mut fen, &mut complete, &mut entries)?;
        }
        if !complete {
            return None;
        }
        Some(FutureStore { fen: fen?, entries })
    }

    /// Compact binary form of this store. This is a cache of the text evidence,
    /// not a replacement for the future-search file (Step 15.5).
    pub fn to_evidence_bytes(&self) -> Vec<u8> {
        let mut keys: Vec<&String> = self.entries.keys().collect();
        keys.sort();
        let mut out = Vec::new();
        push_u32(&mut out, self.fen.len() as u32);
        out.extend_from_slice(self.fen.as_bytes());
        push_u32(&mut out, keys.len() as u32);
        for key in keys {
            push_u32(&mut out, key.len() as u32);
            out.extend_from_slice(key.as_bytes());
            let evidence = &self.entries[key];
            for color in 0..2 {
                for kind in 0..2 {
                    out.extend_from_slice(&evidence.first[color][kind]);
                }
            }
        }
        out
    }

    /// Inverse of [`to_evidence_bytes`](Self::to_evidence_bytes). A short or
    /// corrupt buffer is `None`, so the caller falls back to the text file.
    pub fn from_evidence_bytes(bytes: &[u8]) -> Option<FutureStore> {
        let mut i = 0;
        let fen_len = take_u32(bytes, &mut i)? as usize;
        if fen_len > bytes.len().saturating_sub(i) {
            return None;
        }
        let fen = std::str::from_utf8(bytes.get(i..i + fen_len)?).ok()?.to_string();
        i += fen_len;
        let count = take_u32(bytes, &mut i)? as usize;
        let min_entry = 4 + EVIDENCE_BYTES;
        if count > bytes.len().saturating_sub(i) / min_entry {
            return None;
        }
        let mut entries = HashMap::with_capacity(count);
        for _ in 0..count {
            let key_len = take_u32(bytes, &mut i)? as usize;
            if key_len > bytes.len().saturating_sub(i) {
                return None;
            }
            let key = std::str::from_utf8(bytes.get(i..i + key_len)?).ok()?.to_string();
            i += key_len;
            let raw = bytes.get(i..i + EVIDENCE_BYTES)?;
            i += EVIDENCE_BYTES;
            let mut evidence = Evidence::default();
            let mut at = 0;
            for color in 0..2 {
                for kind in 0..2 {
                    evidence.first[color][kind].copy_from_slice(&raw[at..at + N]);
                    at += N;
                }
            }
            entries.insert(key, evidence);
        }
        if i != bytes.len() {
            return None;
        }
        Some(FutureStore { fen, entries })
    }
}

/// `first[color][kind][strategy]`: two colors, two kinds, [`N`] strategies.
const EVIDENCE_BYTES: usize = 2 * 2 * N;

fn push_u32(out: &mut Vec<u8>, n: u32) {
    out.extend_from_slice(&n.to_le_bytes());
}

fn take_u32(bytes: &[u8], i: &mut usize) -> Option<u32> {
    let raw = bytes.get(*i..*i + 4)?;
    *i += 4;
    Some(u32::from_le_bytes(raw.try_into().ok()?))
}

/// Split a future file into the header and the evidence section when the
/// sequence listing in between cannot hold an evidence line.
fn evidence_regions(text: &[u8]) -> (&[u8], &[u8]) {
    let Some(seq) = find_bytes(text, b"\nEvery sequence of up to ") else {
        return (text, &[]);
    };
    let Some(rel) = find_bytes(&text[seq..], b"STORED STRATEGY EVIDENCE") else {
        return (text, &[]);
    };
    let marker = seq + rel;
    if region_has_e_line(&text[seq..marker]) {
        (text, &[])
    } else {
        (&text[..seq], &text[marker..])
    }
}

fn region_has_e_line(region: &[u8]) -> bool {
    region.starts_with(b"E ") || find_bytes(region, b"\nE ").is_some()
}

fn find_bytes(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

fn rfind_bytes(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    hay.windows(needle.len()).rposition(|w| w == needle)
}

fn parse_evidence_lines(
    text: &[u8],
    fen: &mut Option<String>,
    complete: &mut bool,
    entries: &mut HashMap<String, Evidence>,
) -> Option<()> {
    let mut i = 0;
    while i < text.len() {
        let newline = text[i..].iter().position(|&c| c == b'\n').map(|p| i + p);
        let end = newline.unwrap_or(text.len());
        let mut line = &text[i..end];
        if line.last() == Some(&b'\r') {
            line = &line[..line.len() - 1];
        }
        i = match newline {
            Some(n) => n + 1,
            None => text.len(),
        };
        if let Some(rest) = line.strip_prefix(b"Position: ") {
            *fen = Some(utf8_trimmed(rest)?);
        } else if line.starts_with(b"Status: complete") {
            *complete = true;
        } else if let Some(rest) = line.strip_prefix(b"E ") {
            let w = rfind_bytes(rest, b" W ")?;
            let key = &rest[..w];
            let parts = split_ws(&rest[w + 1..]);
            if parts.len() != 6 || parts[0] != b"W" || parts[3] != b"B" {
                return None;
            }
            let mut evidence = Evidence::default();
            evidence.first[0][0] = parse_digits(parts[1])?;
            evidence.first[0][1] = parse_digits(parts[2])?;
            evidence.first[1][0] = parse_digits(parts[4])?;
            evidence.first[1][1] = parse_digits(parts[5])?;
            let key = if key == b"-" { String::new() } else { utf8_trimmed(key)? };
            entries.insert(key, evidence);
        }
    }
    Some(())
}

fn utf8_trimmed(bytes: &[u8]) -> Option<String> {
    let bytes = trim_ascii(bytes);
    Some(std::str::from_utf8(bytes).ok()?.to_string())
}

fn trim_ascii(mut bytes: &[u8]) -> &[u8] {
    while bytes.first().is_some_and(|c| c.is_ascii_whitespace()) {
        bytes = &bytes[1..];
    }
    while bytes.last().is_some_and(|c| c.is_ascii_whitespace()) {
        bytes = &bytes[..bytes.len() - 1];
    }
    bytes
}

fn split_ws(bytes: &[u8]) -> Vec<&[u8]> {
    bytes.split(|c| c.is_ascii_whitespace()).filter(|part| !part.is_empty()).collect()
}

fn parse_digits(bytes: &[u8]) -> Option<[u8; N]> {
    if bytes.len() != N {
        return None;
    }
    let mut digits = [0u8; N];
    for (i, c) in bytes.iter().enumerate() {
        digits[i] = c.checked_sub(b'0').filter(|d| *d <= 9)?;
    }
    Some(digits)
}

/// Subtree sizes for one first move. `counts[d]` is the number of sequences of
/// length `d + 1` that start with this move.
#[derive(Clone, Debug)]
pub struct RootLine {
    pub san: String,
    pub counts: [u64; SEARCH_PLIES],
}

#[derive(Clone, Debug)]
pub struct SearchReport {
    pub fen: String,
    pub to_move: Color,
    /// `counts[d]`: number of legal sequences of exactly `d + 1` individual moves.
    pub counts: [u64; SEARCH_PLIES],
    pub roots: Vec<RootLine>,
    /// Every sequence of up to `LISTED_PLIES` moves, in SAN.
    pub lines: Vec<String>,
    /// Strategy evidence for Lists 1, 3 and 4.
    pub store: FutureStore,
    pub elapsed: Duration,
    pub threads: usize,
}

impl SearchReport {
    pub fn total_nodes(&self) -> u64 {
        self.counts.iter().sum()
    }
}

/// One position's strategy readings, as bit masks, for both players.
struct Reading {
    progress: [u32; 2],
    completion: [u32; 2],
}

fn read_node(board: &Board, moves: &[RecordedMove]) -> Reading {
    let mut out = Reading { progress: [0; 2], completion: [0; 2] };
    for color in Color::BOTH {
        let ctx = EvalContext::new(board, color, moves);
        let (mut p, mut c) = (0u32, 0u32);
        for (i, def) in STRATEGIES.iter().enumerate() {
            let r = (def.read)(&ctx);
            if r.completion {
                p |= 1 << i;
                c |= 1 << i;
            } else if r.progress {
                p |= 1 << i;
            }
        }
        out.progress[color.index()] = p;
        out.completion[color.index()] = c;
    }
    out
}

struct Walker<'a> {
    progress: &'a Progress,
    moves: Vec<RecordedMove>,
    path: Vec<String>,
    counts: [u64; SEARCH_PLIES],
    entries: HashMap<String, Evidence>,
    read_plies: usize,
}

impl Walker<'_> {
    /// Walk below `board`, which stands `depth` individual moves from the
    /// searched position (`depth` is at most `SEARCH_PLIES - 1`). Adds the
    /// number of sequences to `counts` and returns this position's evidence:
    /// the earliest ply, on any line below it, at which each strategy shows
    /// progress or completion. Returns `None` if cancelled.
    fn visit(&mut self, board: &Board, depth: usize) -> Option<Evidence> {
        if depth <= 3 && self.progress.cancelled() {
            return None;
        }
        if depth == SEARCH_PLIES - 1 && self.read_plies < SEARCH_PLIES {
            self.counts[depth] += board.count_legal_moves();
            return Some(Evidence::default());
        }
        let mut legal = Vec::with_capacity(64);
        board.legal_moves_into(&mut legal);
        self.counts[depth] += legal.len() as u64;

        // A line that has ended (mate, stalemate, a draw rule) is not read further.
        let live = depth < self.read_plies && !board.is_game_over();
        let mut here = Evidence::default();
        for m in &legal {
            let next = board.play(m);
            let child_depth = depth + 1;
            let keep_move = live;
            let keep_path = child_depth <= 2;
            if keep_move {
                self.moves.push(RecordedMove::without_san(m));
            }
            if keep_path {
                self.path.push(m.uci());
            }
            let reading = if live { Some(read_node(&next, &self.moves)) } else { None };
            let child = if child_depth < SEARCH_PLIES { self.visit(&next, child_depth) } else { Some(Evidence::default()) };
            if keep_path {
                self.path.pop();
            }
            if keep_move {
                self.moves.pop();
            }
            let child = child?;
            if let Some(r) = reading {
                here.absorb(&r, if child_depth < SEARCH_PLIES { Some(&child) } else { None });
            }
        }
        if depth <= 2 {
            self.entries.insert(FutureStore::key(&self.path), here);
        }
        Some(here)
    }
}

/// For one player: the readings of the 19 strategies that look only at the
/// board (progress-or-completion bits, completion bits), and whether central
/// pawn break's progress condition holds (it reads only the board; its
/// completion condition reads the history window and is checked on every line).
type BoardReading = (u32, u32, bool);

/// The readings of one position for White and for Black, shared by all the
/// search threads, in shards so they rarely wait for each other.
struct ReadCache {
    shards: Vec<Mutex<HashMap<[u64; 4], [BoardReading; 2], BuildFast>>>,
}

const SHARDS: usize = 64;

/// Positions kept in the shared cache (about 70 bytes each); a full shard is
/// emptied and refilled.
const CACHE_LIMIT: usize = 8_000_000;

impl ReadCache {
    fn new() -> ReadCache {
        ReadCache { shards: (0..SHARDS).map(|_| Mutex::new(HashMap::default())).collect() }
    }

    fn shard(&self, key: &[u64; 4]) -> &Mutex<HashMap<[u64; 4], [BoardReading; 2], BuildFast>> {
        let h = (key[0] ^ key[1].rotate_left(17) ^ key[2].rotate_left(31) ^ key[3]).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        &self.shards[(h >> 58) as usize % SHARDS]
    }

    fn get(&self, key: &[u64; 4]) -> Option<[BoardReading; 2]> {
        self.shard(key).lock().unwrap().get(key).copied()
    }

    fn put(&self, key: [u64; 4], v: [BoardReading; 2]) {
        let mut shard = self.shard(&key).lock().unwrap();
        if shard.len() >= CACHE_LIMIT / SHARDS {
            shard.clear();
        }
        shard.insert(key, v);
    }
}

/// A fast hash for the cache keys (the keys are already well mixed bitboards).
#[derive(Default, Clone, Copy)]
struct BuildFast;

impl std::hash::BuildHasher for BuildFast {
    type Hasher = FastHasher;
    fn build_hasher(&self) -> FastHasher {
        FastHasher(0)
    }
}

struct FastHasher(u64);

impl std::hash::Hasher for FastHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut b = [0u8; 8];
            b[..chunk.len()].copy_from_slice(chunk);
            self.write_u64(u64::from_le_bytes(b));
        }
    }
    fn write_u64(&mut self, v: u64) {
        self.0 = (self.0.rotate_left(5) ^ v).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    fn write_u8(&mut self, v: u8) {
        self.write_u64(v as u64);
    }
    fn write_usize(&mut self, v: usize) {
        self.write_u64(v as u64);
    }
}

/// Central pawn break's completion condition (Step 14.4, strategy 2): a pawn
/// capture of a pawn by `side`, landing on the c-, d-, e- or f-file, in that
/// player's history window (the previous four turns, and the current one when
/// `side` has just moved; rule 14.1.4), read from the end of `moves`.
fn central_break_in_window(moves: &[RecordedMove], side: Color, turn: Color) -> bool {
    let count = if turn == side { 4 } else { 5 };
    moves
        .iter()
        .rev()
        .filter(|m| m.color == side)
        .take(count)
        .any(|m| m.piece == crate::board::PieceKind::Pawn && m.captured == Some(crate::board::PieceKind::Pawn) && (2..=5).contains(&crate::board::file_of(m.to)))
}

/// Read all 20 strategies for `color` once: the 19 board-only bits, and central
/// pawn break's board-only progress condition.
fn read_board_only(board: &Board, color: Color, moves: &[RecordedMove], history: u32) -> (u32, u32, bool) {
    let ctx = EvalContext::flags_only(board, color, moves);
    let (mut p, mut c, mut break_progress) = (0u32, 0u32, false);
    for (i, def) in STRATEGIES.iter().enumerate() {
        let r = (def.read)(&ctx);
        if 1 << i == history {
            break_progress = r.progress;
            continue;
        }
        if r.completion {
            p |= 1 << i;
            c |= 1 << i;
        } else if r.progress {
            p |= 1 << i;
        }
    }
    (p, c, break_progress)
}

/// The bit of central pawn break, the one strategy that reads the history window.
fn history_bit() -> u32 {
    let i = STRATEGIES.iter().position(|d| d.id == "central-pawn-break").expect("central pawn break exists");
    1 << i
}

/// The six-move search, reading strategies only where a reading can still
/// change a stored result.
///
/// `best[a]` is the evidence collected so far for the stored position at depth
/// `a` on the current line (0 = this first move's share of the searched
/// position, 1 = the position after one move, 2 = after a move and a reply):
/// for each player, kind (progress, completion) and strategy, the smallest
/// number of individual moves below that position at which it was read, or 0
/// if not yet. A reading at depth `d` offers `d - a` to each stored position
/// above it, so it is needed only where one of them holds 0 or a larger number.
struct Exact<'a> {
    progress: &'a Progress,
    moves: Vec<RecordedMove>,
    path: Vec<String>,
    counts: [u64; SEARCH_PLIES],
    entries: HashMap<String, Evidence>,
    best: [Evidence; 3],
    cache: &'a ReadCache,
    history: u32,
}

impl<'a> Exact<'a> {
    fn new(progress: &'a Progress, game_moves: &[RecordedMove], first: String, cache: &'a ReadCache) -> Exact<'a> {
        Exact {
            progress,
            moves: game_moves.to_vec(),
            path: vec![first],
            counts: [0; SEARCH_PLIES],
            entries: HashMap::new(),
            best: [Evidence::default(); 3],
            cache,
            history: history_bit(),
        }
    }

    /// The strategies worth reading on a position at depth `d`, per player.
    fn needed(&self, d: usize) -> [u32; 2] {
        let mut need = [0u32; 2];
        for a in 0..d.min(3) {
            let offer = (d - a) as u8;
            let f = &self.best[a].first;
            for (c, slot) in need.iter_mut().enumerate() {
                for kind in &f[c] {
                    for (i, &have) in kind.iter().enumerate() {
                        if have == 0 || have > offer {
                            *slot |= 1 << i;
                        }
                    }
                }
            }
        }
        need
    }

    /// The needed strategies on `board`, for each player that needs any: the
    /// board-only readings from the cache (all 20 are read once on a miss), and
    /// central pawn break's completion from the history window.
    fn read_needed(&mut self, board: &Board, need: [u32; 2]) -> Reading {
        let mut out = Reading { progress: [0; 2], completion: [0; 2] };
        let key = board.packed_key();
        let cached = key.and_then(|k| self.cache.get(&k));
        let readings = match cached {
            Some(v) => v,
            None => {
                let v = Color::BOTH.map(|c| read_board_only(board, c, &self.moves, self.history));
                if let Some(k) = key {
                    self.cache.put(k, v);
                }
                v
            }
        };
        for color in Color::BOTH {
            let ci = color.index();
            if need[ci] == 0 {
                continue;
            }
            let (p, c, break_progress) = readings[ci];
            out.progress[ci] |= p;
            out.completion[ci] |= c;
            if central_break_in_window(&self.moves, color, board.turn()) {
                out.progress[ci] |= self.history;
                out.completion[ci] |= self.history;
            } else if break_progress {
                out.progress[ci] |= self.history;
            }
        }
        out
    }

    /// Read `board` (at depth `d`, its move already on `moves`) where needed and
    /// offer the result to every stored position above it. Offering a reading
    /// that was not needed changes nothing.
    fn read(&mut self, board: &Board, d: usize) {
        let need = self.needed(d);
        if need == [0, 0] {
            return;
        }
        let r = self.read_needed(board, need);
        for a in 0..d.min(3) {
            let offer = (d - a) as u8;
            let f = &mut self.best[a].first;
            for c in 0..2 {
                for (k, mask) in [r.progress[c], r.completion[c]].into_iter().enumerate() {
                    if mask == 0 {
                        continue;
                    }
                    for i in 0..N {
                        if mask & (1 << i) != 0 {
                            f[c][k][i] = min_nonzero(f[c][k][i], offer);
                        }
                    }
                }
            }
        }
    }

    /// Walk below `board` at `depth` (1..=5). The moves out of a position whose
    /// game has ended (mate, stalemate, a draw rule) are counted, not read.
    fn visit(&mut self, board: &Board, depth: usize) -> Option<()> {
        if depth <= 3 && self.progress.cancelled() {
            return None;
        }
        if depth <= 2 {
            self.best[depth] = Evidence::default();
        }
        let mut legal = Vec::with_capacity(64);
        board.legal_moves_into(&mut legal);
        self.counts[depth] += legal.len() as u64;
        let live = !board.is_game_over();
        for m in &legal {
            let next = board.play(m);
            let child_depth = depth + 1;
            let keep_path = child_depth <= 2;
            if keep_path {
                self.path.push(m.uci());
            }
            if live {
                self.moves.push(RecordedMove::without_san(m));
                self.read(&next, child_depth);
            }
            if child_depth < SEARCH_PLIES {
                self.visit(&next, child_depth)?;
            }
            if live {
                self.moves.pop();
            }
            if keep_path {
                self.path.pop();
            }
        }
        if depth <= 2 {
            self.entries.insert(FutureStore::key(&self.path), self.best[depth]);
        }
        Some(())
    }
}

fn list_lines(board: &Board, prefix: &mut Vec<String>, depth_left: usize, out: &mut Vec<String>) {
    let legal = board.legal_moves();
    if legal.is_empty() {
        if !prefix.is_empty() {
            let end = if board.is_check() { "(checkmate)" } else { "(stalemate)" };
            out.push(format!("{} {}", prefix.join(" "), end));
        }
        return;
    }
    if depth_left == 0 {
        out.push(prefix.join(" "));
        return;
    }
    for m in &legal {
        prefix.push(board.san_with(m, &legal));
        list_lines(&board.play(m), prefix, depth_left - 1, out);
        prefix.pop();
    }
}

struct RootResult {
    counts: [u64; SEARCH_PLIES],
    entries: HashMap<String, Evidence>,
    /// This first move's share of the searched position's evidence.
    contribution: Evidence,
}

/// Run the full six-ply search from `board`. `game_moves` is the move record
/// that led to `board` (it feeds the history window that Central Pawn Break
/// reads). The strategies are read through all six individual moves. Returns
/// `None` if the search was cancelled.
pub fn full_search(board: &Board, game_moves: &[RecordedMove], progress: &Progress) -> Option<SearchReport> {
    search_with(board, game_moves, progress, None)
}

/// The same search, reading every one of the 20 strategies for both players on
/// every position from individual move 1 to `read_plies`, with nothing skipped
/// or reused. Slow; the tests use it to prove that [`full_search`] stores
/// exactly the same evidence.
pub fn full_search_reference(board: &Board, game_moves: &[RecordedMove], progress: &Progress, read_plies: usize) -> Option<SearchReport> {
    search_with(board, game_moves, progress, Some(read_plies.min(SEARCH_PLIES)))
}

fn search_with(board: &Board, game_moves: &[RecordedMove], progress: &Progress, reference: Option<usize>) -> Option<SearchReport> {
    let start = Instant::now();
    let legal = board.legal_moves();
    progress.roots_total.store(legal.len(), Ordering::Relaxed);
    progress.roots_done.store(0, Ordering::Relaxed);

    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).min(legal.len().max(1));
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<RootResult>>> = Mutex::new((0..legal.len()).map(|_| None).collect());
    let live_root = !board.is_game_over();

    let cache = ReadCache::new();
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= legal.len() || progress.cancelled() {
                        return;
                    }
                    let m = &legal[i];
                    let child = board.play(m);
                    let result = match reference {
                        Some(read_plies) => {
                            let mut walker = Walker {
                                progress,
                                moves: game_moves.to_vec(),
                                path: vec![m.uci()],
                                counts: [0; SEARCH_PLIES],
                                entries: HashMap::new(),
                                read_plies,
                            };
                            walker.counts[0] = 1;
                            walker.moves.push(RecordedMove::without_san(m));
                            let reading = if live_root && read_plies >= 1 { Some(read_node(&child, &walker.moves)) } else { None };
                            let Some(child_ev) = walker.visit(&child, 1) else { return };
                            let mut contribution = Evidence::default();
                            if let Some(r) = reading {
                                contribution.absorb(&r, Some(&child_ev));
                            }
                            RootResult { counts: walker.counts, entries: walker.entries, contribution }
                        }
                        None => {
                            let mut w = Exact::new(progress, game_moves, m.uci(), &cache);
                            w.counts[0] = 1;
                            w.moves.push(RecordedMove::without_san(m));
                            if live_root {
                                w.read(&child, 1);
                            }
                            let Some(()) = w.visit(&child, 1) else { return };
                            RootResult { counts: w.counts, entries: w.entries, contribution: w.best[0] }
                        }
                    };
                    results.lock().unwrap()[i] = Some(result);
                    progress.roots_done.fetch_add(1, Ordering::Relaxed);
                }
            });
        }
    });
    if progress.cancelled() {
        return None;
    }

    let mut totals = [0u64; SEARCH_PLIES];
    let mut roots = Vec::with_capacity(legal.len());
    let mut entries: HashMap<String, Evidence> = HashMap::new();
    let mut root_evidence = Evidence::default();
    for (m, result) in legal.iter().zip(results.into_inner().unwrap()) {
        let result = result?;
        for d in 0..SEARCH_PLIES {
            totals[d] += result.counts[d];
        }
        // Merge this move's share: earliest occurrence over all root moves.
        for c in 0..2 {
            for k in 0..2 {
                for i in 0..N {
                    root_evidence.first[c][k][i] = min_nonzero(root_evidence.first[c][k][i], result.contribution.first[c][k][i]);
                }
            }
        }
        entries.extend(result.entries);
        roots.push(RootLine { san: board.san_with(m, &legal), counts: result.counts });
    }
    entries.insert(String::new(), root_evidence);

    let mut lines = Vec::new();
    list_lines(board, &mut Vec::new(), LISTED_PLIES, &mut lines);

    let fen = board.fen();
    Some(SearchReport {
        fen: fen.clone(),
        to_move: board.turn(),
        counts: totals,
        roots,
        lines,
        store: FutureStore { fen, entries },
        elapsed: start.elapsed(),
        threads,
    })
}

/// Group digits with commas: 124132536 -> "124,132,536".
pub fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Which player makes individual move number `ply + 1` of the search.
pub fn mover_at(to_move: Color, ply: usize) -> Color {
    if ply % 2 == 0 {
        to_move
    } else {
        to_move.other()
    }
}
