//! Development scores and Lists 1–4 (Steps 8–13; port of `evaluate.ts`).

use std::cmp::Ordering;

use crate::board::{square_name, Board, Color, Move, PieceKind, Square};
use crate::exchange::{value_capture, CaptureValuation};
use crate::geometry::{Unit, MATE_BONUS};
use crate::position::{EvalContext, RecordedMove};
use crate::search::FutureStore;
use crate::strategies::{StrategyDefinition, StrategyReading, STRATEGIES};

#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum Stage {
    Full,
    Partial,
    Beginning,
}

impl Stage {
    pub fn label(self) -> &'static str {
        match self {
            Stage::Full => "Fully developed",
            Stage::Partial => "Partially developed",
            Stage::Beginning => "Beginning to form",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScoredMilestones {
    pub stage: Stage,
    pub score: u32,
    pub progress_within_turn: bool,
    pub completion_within_turn: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FutureFlags {
    pub progress: bool,
    pub completion: bool,
}

/// Development score for one strategy on one position (Step 14.2).
///
/// * Completion now: fully developed, 100.
/// * Foundation and progress now: 60, plus 15 when completion appears within one full turn.
/// * Foundation now plus progress within one full turn: 45.
/// * Foundation alone, or progress alone: 30.
/// * A future line never creates a strategy by itself.
pub fn score_milestones(now: &StrategyReading, future: FutureFlags) -> Option<ScoredMilestones> {
    if now.completion {
        return Some(ScoredMilestones { stage: Stage::Full, score: 100, progress_within_turn: false, completion_within_turn: false });
    }
    if !now.foundation && !now.progress {
        return None;
    }
    let progress_within_turn = now.foundation && !now.progress && (future.progress || future.completion);
    let completion_within_turn = now.foundation && now.progress && future.completion;
    let mut score = 0;
    if now.foundation {
        score += 30;
    }
    if now.progress {
        score += 30;
    } else if progress_within_turn {
        score += 15;
    }
    if completion_within_turn {
        score += 15;
    }
    Some(ScoredMilestones {
        stage: if now.foundation && now.progress { Stage::Partial } else { Stage::Beginning },
        score,
        progress_within_turn,
        completion_within_turn,
    })
}

pub fn explain_score(scored: &ScoredMilestones, now: &StrategyReading) -> &'static str {
    if now.completion {
        "Completion is true on this position, so the strategy is fully developed."
    } else if scored.completion_within_turn {
        "Foundation and progress are already on the board, and a legal line within one full turn reaches completion. That is partial, score 75, until the completing move is the position itself."
    } else if now.foundation && now.progress {
        "Foundation and progress are on the board. Completion is not, and it does not appear within one full turn."
    } else if scored.progress_within_turn {
        "The foundation is on the board. The progress condition is not met yet, but a legal line within one full turn reaches it."
    } else if now.foundation {
        "Only the foundation is present. The progress move is not available within one full turn."
    } else {
        "Progress is visible without the foundation. The plan has started, and the structural base is not on the board."
    }
}

/// One row of List 2 / List 4.
#[derive(Clone, Debug)]
pub struct CaptureRow {
    pub strategy_id: &'static str,
    pub strategy_name: &'static str,
    pub san: String,
    pub from: Square,
    pub to: Square,
    pub capturer: PieceKind,
    pub victim: PieceKind,
    pub victim_square: Square,
    pub expected_gain: i64,
    pub potential_loss: i64,
    pub net: i64,
    pub mate: bool,
    pub recapture_sans: Vec<String>,
}

/// Every legal capture, priced, whether or not a strategy claims it.
#[derive(Clone, Debug)]
pub struct ValuedCapture {
    pub san: String,
    pub from: Square,
    pub to: Square,
    pub capturer: PieceKind,
    pub victim: PieceKind,
    pub victim_square: Square,
    pub valuation: CaptureValuation,
    pub strategies: Vec<&'static str>,
}

/// One row of List 1 / List 3.
#[derive(Clone, Debug)]
pub struct StrategyHit {
    pub definition: &'static StrategyDefinition,
    pub id: &'static str,
    pub name: &'static str,
    pub stage: Stage,
    pub score: u32,
    pub foundation: bool,
    pub progress: bool,
    pub completion: bool,
    pub progress_within_turn: bool,
    pub completion_within_turn: bool,
    pub explanation: &'static str,
    pub evidence: String,
    pub participants: Vec<Unit>,
    pub targets: Vec<Unit>,
    pub captures: Vec<CaptureRow>,
    /// From the stored search: the earliest individual move, counted from the
    /// position scored, at which progress / completion appears on some line
    /// anywhere in the rest of the six-move search. `None` = never (or no
    /// stored search). This is extra information: the score uses one full turn.
    pub progress_in: Option<u8>,
    pub completion_in: Option<u8>,
}

#[derive(Clone, Debug)]
pub struct Evaluation {
    pub side: Color,
    pub turn: Color,
    pub fen: String,
    pub side_to_move: bool,
    /// List 1 (or List 3), sorted.
    pub hits: Vec<StrategyHit>,
    /// List 2 (or List 4), sorted.
    pub captures: Vec<CaptureRow>,
    pub legal_captures: Vec<ValuedCapture>,
}

/// Approximates JavaScript `localeCompare` for the ASCII text used here:
/// compare case-insensitively first, then put lower case before upper case.
pub fn locale_cmp(a: &str, b: &str) -> Ordering {
    let la = a.to_ascii_lowercase();
    let lb = b.to_ascii_lowercase();
    la.cmp(&lb).then_with(|| {
        for (x, y) in a.chars().zip(b.chars()) {
            if x != y {
                return if x.is_ascii_lowercase() { Ordering::Less } else { Ordering::Greater };
            }
        }
        a.len().cmp(&b.len())
    })
}

fn sort_hits(hits: &mut [StrategyHit]) {
    hits.sort_by(|a, b| {
        a.stage
            .cmp(&b.stage)
            .then(b.score.cmp(&a.score))
            .then_with(|| locale_cmp(a.name, b.name))
    });
}

pub fn sort_captures(rows: &mut [CaptureRow]) {
    rows.sort_by(|a, b| {
        b.net
            .cmp(&a.net)
            .then(b.expected_gain.cmp(&a.expected_gain))
            .then(a.potential_loss.cmp(&b.potential_loss))
            .then_with(|| locale_cmp(a.strategy_name, b.strategy_name))
            .then_with(|| locale_cmp(&a.san, &b.san))
            .then_with(|| square_name(a.victim_square).cmp(&square_name(b.victim_square)))
    });
}

fn associate_captures(ctx: &EvalContext, def: &'static StrategyDefinition, reading: &StrategyReading) -> Vec<CaptureRow> {
    if ctx.board.turn() != ctx.side {
        return vec![];
    }
    let participants: Vec<Square> = reading.participants.iter().filter(|u| u.color == ctx.side).map(|u| u.square).collect();
    let targets: Vec<Square> = reading.targets.iter().map(|u| u.square).collect();
    let legal = ctx.board.legal_moves();
    let mut rows = Vec::new();
    for m in &legal {
        if !m.is_capture() || !participants.contains(&m.from) {
            continue;
        }
        let victim_square = m.victim_square();
        if !targets.contains(&victim_square) {
            continue;
        }
        let v = value_capture(ctx.board, m);
        rows.push(CaptureRow {
            strategy_id: def.id,
            strategy_name: def.name,
            san: ctx.board.san_with(m, &legal),
            from: m.from,
            to: m.to,
            capturer: m.piece,
            victim: m.captured.unwrap(),
            victim_square,
            expected_gain: v.expected_gain,
            potential_loss: v.potential_loss,
            net: v.net,
            mate: v.mate,
            recapture_sans: v.recapture_sans,
        });
    }
    rows
}

fn needs_future(reading: &StrategyReading, future: &FutureFlags) -> bool {
    if reading.completion || !reading.foundation {
        return false;
    }
    if reading.progress {
        !future.completion
    } else {
        !future.progress
    }
}

/// Look one full turn (two plies) ahead and record whether progress or
/// completion of each unfinished strategy can be reached.
fn search_one_full_turn(board: &Board, side: Color, moves: &[RecordedMove], now: &[StrategyReading], future: &mut [FutureFlags]) {
    fn unfinished(now: &[StrategyReading], future: &[FutureFlags]) -> bool {
        now.iter().zip(future.iter()).any(|(r, f)| needs_future(r, f))
    }
    fn absorb(board: &Board, side: Color, moves: &[RecordedMove], now: &[StrategyReading], future: &mut [FutureFlags]) {
        let child = EvalContext::new(board, side, moves);
        for (i, def) in STRATEGIES.iter().enumerate() {
            if !needs_future(&now[i], &future[i]) {
                continue;
            }
            let r = (def.read)(&child);
            if r.completion {
                future[i].completion = true;
                future[i].progress = true;
            } else if r.progress {
                future[i].progress = true;
            }
        }
    }
    fn walk(board: &Board, side: Color, moves: &mut Vec<RecordedMove>, plies: u32, now: &[StrategyReading], future: &mut [FutureFlags]) {
        if plies == 0 || !unfinished(now, future) || board.is_game_over() {
            return;
        }
        for m in board.legal_moves() {
            if !unfinished(now, future) {
                return;
            }
            let next = board.play(&m);
            moves.push(RecordedMove::without_san(&m));
            absorb(&next, side, moves, now, future);
            if !next.is_game_over() {
                walk(&next, side, moves, plies - 1, now, future);
            }
            moves.pop();
        }
    }
    let mut line = moves.to_vec();
    walk(board, side, &mut line, 2, now, future);
}

/// List 1 and List 2 for `side` on this position, with the one-full-turn
/// evidence found by a small in-memory walk. Used by tests and by library
/// callers that have no stored search. The game itself uses
/// [`evaluate_stored`], which reads the Step 6 search instead.
pub fn evaluate_position(board: &Board, side: Color, moves: &[RecordedMove]) -> Evaluation {
    evaluate_core(board, side, moves, None)
}

/// List 1 and List 2 (or List 3 and List 4 on a later position), reading the
/// one-full-turn evidence from the stored Step 6 search. No new search is run.
/// `path` is the UCI move sequence from the position the search started on to
/// `board` (empty for the search's own position).
pub fn evaluate_stored(board: &Board, side: Color, moves: &[RecordedMove], store: &FutureStore, path: &[String]) -> Evaluation {
    let ev = store.get(path).copied().unwrap_or_default();
    evaluate_core(board, side, moves, Some((ev.flags(side), ev.horizon(side))))
}

fn evaluate_core(board: &Board, side: Color, moves: &[RecordedMove], stored: Option<(Vec<FutureFlags>, Vec<(u8, u8)>)>) -> Evaluation {
    let ctx = EvalContext::new(board, side, moves);
    let now: Vec<StrategyReading> = STRATEGIES.iter().map(|d| (d.read)(&ctx)).collect();
    let (future, horizon) = match stored {
        Some((f, h)) => (f, Some(h)),
        None => {
            let mut future = vec![FutureFlags::default(); STRATEGIES.len()];
            if now.iter().zip(future.iter()).any(|(r, f)| needs_future(r, f)) {
                search_one_full_turn(board, side, moves, &now, &mut future);
            }
            (future, None)
        }
    };

    let mut hits = Vec::new();
    for (i, def) in STRATEGIES.iter().enumerate() {
        let reading = &now[i];
        let Some(scored) = score_milestones(reading, future[i]) else { continue };
        hits.push(StrategyHit {
            definition: def,
            id: def.id,
            name: def.name,
            stage: scored.stage,
            score: scored.score,
            foundation: reading.foundation,
            progress: reading.progress,
            completion: reading.completion,
            progress_within_turn: scored.progress_within_turn,
            completion_within_turn: scored.completion_within_turn,
            explanation: explain_score(&scored, reading),
            evidence: reading.evidence.clone(),
            participants: reading.participants.clone(),
            targets: reading.targets.clone(),
            captures: associate_captures(&ctx, def, reading),
            progress_in: horizon.as_ref().map(|h| h[i].0).filter(|&p| p > 0),
            completion_in: horizon.as_ref().map(|h| h[i].1).filter(|&c| c > 0),
        });
    }
    sort_hits(&mut hits);
    let mut captures: Vec<CaptureRow> = hits.iter().flat_map(|h| h.captures.iter().cloned()).collect();
    sort_captures(&mut captures);
    let legal_captures = value_legal_captures(board, side, &hits);

    Evaluation { side, turn: board.turn(), fen: board.fen(), side_to_move: board.turn() == side, hits, captures, legal_captures }
}

fn value_legal_captures(board: &Board, side: Color, hits: &[StrategyHit]) -> Vec<ValuedCapture> {
    if board.turn() != side {
        return vec![];
    }
    let legal = board.legal_moves();
    let mut rows: Vec<ValuedCapture> = legal
        .iter()
        .filter(|m| m.is_capture())
        .map(|m| {
            let san = board.san_with(m, &legal);
            let strategies = hits
                .iter()
                .filter(|h| h.captures.iter().any(|c| c.from == m.from && c.to == m.to && c.san == san))
                .map(|h| h.name)
                .collect();
            ValuedCapture {
                from: m.from,
                to: m.to,
                capturer: m.piece,
                victim: m.captured.unwrap(),
                victim_square: m.victim_square(),
                valuation: value_capture(board, m),
                strategies,
                san,
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        b.valuation
            .net
            .cmp(&a.valuation.net)
            .then(b.valuation.expected_gain.cmp(&a.valuation.expected_gain))
            .then(a.valuation.potential_loss.cmp(&b.valuation.potential_loss))
            .then_with(|| locale_cmp(&a.san, &b.san))
    });
    rows
}

pub fn evaluate_fen(fen: &str, side: Color, moves: &[RecordedMove]) -> Result<Evaluation, String> {
    Ok(evaluate_position(&Board::from_fen(fen)?, side, moves))
}

/// One future branch for List 3: the next move, the position it produces, and
/// the forming strategies there.
#[derive(Clone, Debug)]
pub struct FutureBranch {
    pub mv: Move,
    pub san: String,
    pub board: Board,
    pub moves: Vec<RecordedMove>,
    /// UCI moves from the searched position to `board`.
    pub path: Vec<String>,
    pub evaluation: Evaluation,
}

/// List 3 for every legal next move of `side` (Step 12). `side` must be the
/// side to move. Each result stays tied to the move and position that produced it.
pub fn list3_all(board: &Board, side: Color, moves: &[RecordedMove]) -> Vec<FutureBranch> {
    list3_all_with(board, side, moves, None)
}

/// Same, reading the evidence from the stored search (no new search).
pub fn list3_all_stored(board: &Board, side: Color, moves: &[RecordedMove], store: &FutureStore) -> Vec<FutureBranch> {
    list3_all_with(board, side, moves, Some(store))
}

fn list3_all_with(board: &Board, side: Color, moves: &[RecordedMove], store: Option<&FutureStore>) -> Vec<FutureBranch> {
    if board.turn() != side {
        return vec![];
    }
    let legal = board.legal_moves();
    legal.iter().map(|m| list3_for(board, side, moves, m, &legal, store)).collect()
}

fn list3_for(board: &Board, side: Color, moves: &[RecordedMove], m: &Move, legal: &[Move], store: Option<&FutureStore>) -> FutureBranch {
    let san = board.san_with(m, legal);
    let next = board.play(m);
    let mut rec = RecordedMove::without_san(m);
    rec.san = san.clone();
    let mut line = moves.to_vec();
    line.push(rec);
    let path = vec![m.uci()];
    let evaluation = match store {
        Some(st) => evaluate_stored(&next, side, &line, st, &path),
        None => evaluate_position(&next, side, &line),
    };
    FutureBranch { mv: *m, san, board: next, moves: line, path, evaluation }
}

/// List 3 after one chosen next move (by SAN).
pub fn list3_after(board: &Board, side: Color, moves: &[RecordedMove], next_san: &str) -> Option<FutureBranch> {
    list3_after_with(board, side, moves, next_san, None)
}

pub fn list3_after_stored(board: &Board, side: Color, moves: &[RecordedMove], next_san: &str, store: &FutureStore) -> Option<FutureBranch> {
    list3_after_with(board, side, moves, next_san, Some(store))
}

fn list3_after_with(board: &Board, side: Color, moves: &[RecordedMove], next_san: &str, store: Option<&FutureStore>) -> Option<FutureBranch> {
    if board.turn() != side {
        return None;
    }
    let m = board.parse_san(next_san)?;
    Some(list3_for(board, side, moves, &m, &board.legal_moves(), store))
}

/// List 4: captures after the next move and one opposing reply, when the
/// original side is to move again (Step 13 / 14.5).
pub fn list4_after(branch: &FutureBranch, reply_san: &str) -> Option<(String, Evaluation)> {
    let reply = branch.board.parse_san(reply_san)?;
    Some(list4_for(branch, &reply, None))
}

pub fn list4_after_stored(branch: &FutureBranch, reply_san: &str, store: &FutureStore) -> Option<(String, Evaluation)> {
    let reply = branch.board.parse_san(reply_san)?;
    Some(list4_for(branch, &reply, Some(store)))
}

fn list4_for(branch: &FutureBranch, reply: &Move, store: Option<&FutureStore>) -> (String, Evaluation) {
    let side = branch.evaluation.side;
    let san = branch.board.san(reply);
    let after = branch.board.play(reply);
    let mut line = branch.moves.clone();
    let mut rec = RecordedMove::without_san(reply);
    rec.san = san.clone();
    line.push(rec);
    let evaluation = match store {
        Some(st) => {
            let mut path = branch.path.clone();
            path.push(reply.uci());
            evaluate_stored(&after, side, &line, st, &path)
        }
        None => evaluate_position(&after, side, &line),
    };
    (san, evaluation)
}

/// List 4 for every legal opposing reply to one List 3 move (Step 13).
pub fn list4_all_stored(branch: &FutureBranch, store: &FutureStore) -> Vec<(String, Evaluation)> {
    let legal = branch.board.legal_moves();
    legal.iter().map(|r| list4_for(branch, r, Some(store))).collect()
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

pub fn format_material(value: i64) -> String {
    if value >= MATE_BONUS {
        let rest = value - MATE_BONUS;
        return if rest > 0 { format!("mate + {rest}") } else { "mate".into() };
    }
    value.to_string()
}

pub fn format_signed(value: i64) -> String {
    if value >= MATE_BONUS {
        return format_material(value);
    }
    if value <= -MATE_BONUS {
        return format!("−{}", format_material(-value));
    }
    if value > 0 {
        format!("+{value}")
    } else {
        value.to_string()
    }
}

pub fn describe_capture(row: &CaptureRow) -> String {
    let reply = if row.recapture_sans.is_empty() {
        " No recapture follows.".to_string()
    } else {
        format!(" Reply: {}.", row.recapture_sans.join(" "))
    };
    let mate = if row.mate { " The sequence includes checkmate." } else { "" };
    format!(
        "{} gains {} and risks {}. Net {}.{}{}",
        row.san,
        format_material(row.expected_gain),
        format_material(row.potential_loss),
        format_signed(row.net),
        reply,
        mate
    )
}
