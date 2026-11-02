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

/// Strategies are read on every position from individual move 1 to this one;
/// individual moves beyond it are still searched and counted.
///
/// Measured on the standard start position (12 threads, release build):
/// 4 plies takes about 5 s, 5 plies about 93 s, and 6 plies more than 10 minutes
/// (reading 119 million leaf positions twice). A middlegame position has 20 to 50
/// times more positions. The search is redone after every move, so 4 is the deepest
/// reading that keeps the game playable.
pub const READ_PLIES: usize = 4;

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
        let mut fen = None;
        let mut complete = false;
        let mut entries = HashMap::new();
        for line in text.lines() {
            if let Some(f) = line.strip_prefix("Position: ") {
                fen = Some(f.trim().to_string());
            } else if line.starts_with("Status: complete") {
                complete = true;
            } else if let Some(rest) = line.strip_prefix("E ") {
                // E <key|-> W <p> <c> B <p> <c>   (the key may hold spaces)
                let w = rest.rfind(" W ")?;
                let key = &rest[..w];
                let parts: Vec<&str> = rest[w + 1..].split_whitespace().collect();
                if parts.len() != 6 || parts[0] != "W" || parts[3] != "B" {
                    return None;
                }
                let parse = |s: &str| -> Option<[u8; N]> {
                    if s.len() != N {
                        return None;
                    }
                    let mut a = [0u8; N];
                    for (i, c) in s.bytes().enumerate() {
                        a[i] = c.checked_sub(b'0').filter(|d| *d <= 9)?;
                    }
                    Some(a)
                };
                let mut e = Evidence::default();
                e.first[0][0] = parse(parts[1])?;
                e.first[0][1] = parse(parts[2])?;
                e.first[1][0] = parse(parts[4])?;
                e.first[1][1] = parse(parts[5])?;
                entries.insert(if key == "-" { String::new() } else { key.to_string() }, e);
            }
        }
        if !complete {
            return None;
        }
        Some(FutureStore { fen: fen?, entries })
    }
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
        if depth == SEARCH_PLIES - 1 && READ_PLIES < SEARCH_PLIES {
            self.counts[depth] += board.count_legal_moves();
            return Some(Evidence::default());
        }
        let mut legal = Vec::with_capacity(64);
        board.legal_moves_into(&mut legal);
        self.counts[depth] += legal.len() as u64;

        // A line that has ended (mate, stalemate, a draw rule) is not read further.
        let live = depth < READ_PLIES && !board.is_game_over();
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
/// reads). Returns `None` if the search was cancelled.
pub fn full_search(board: &Board, game_moves: &[RecordedMove], progress: &Progress) -> Option<SearchReport> {
    let start = Instant::now();
    let legal = board.legal_moves();
    progress.roots_total.store(legal.len(), Ordering::Relaxed);
    progress.roots_done.store(0, Ordering::Relaxed);

    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).min(legal.len().max(1));
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<RootResult>>> = Mutex::new((0..legal.len()).map(|_| None).collect());
    let live_root = !board.is_game_over();

    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= legal.len() || progress.cancelled() {
                    return;
                }
                let m = &legal[i];
                let child = board.play(m);
                let mut walker = Walker {
                    progress,
                    moves: game_moves.to_vec(),
                    path: vec![m.uci()],
                    counts: [0; SEARCH_PLIES],
                    entries: HashMap::new(),
                };
                walker.counts[0] = 1;
                walker.moves.push(RecordedMove::without_san(m));
                let reading = if live_root { Some(read_node(&child, &walker.moves)) } else { None };
                let Some(child_ev) = walker.visit(&child, 1) else { return };
                let mut contribution = Evidence::default();
                if let Some(r) = reading {
                    contribution.absorb(&r, Some(&child_ev));
                }
                let result = RootResult { counts: walker.counts, entries: walker.entries, contribution };
                results.lock().unwrap()[i] = Some(result);
                progress.roots_done.fetch_add(1, Ordering::Relaxed);
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
