//! Recorded moves, the history window and the evaluation context (port of `position.ts`).

use crate::board::{Board, Color, Move, PieceKind, Square};
use crate::geometry::Position;

/// A move as it was played, kept for the turn window and the history files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordedMove {
    pub color: Color,
    pub from: Square,
    pub to: Square,
    pub piece: PieceKind,
    pub captured: Option<PieceKind>,
    pub promotion: Option<PieceKind>,
    pub san: String,
    pub en_passant: bool,
}

impl RecordedMove {
    /// Record a move played from `board`, including its SAN.
    pub fn new(board: &Board, m: &Move) -> RecordedMove {
        let mut rec = RecordedMove::without_san(m);
        rec.san = board.san(m);
        rec
    }

    /// Record a move without computing SAN. Used inside searches, where only
    /// the colour, piece, capture and destination matter.
    pub fn without_san(m: &Move) -> RecordedMove {
        RecordedMove {
            color: m.color,
            from: m.from,
            to: m.to,
            piece: m.piece,
            captured: m.captured,
            promotion: m.promotion,
            san: String::new(),
            en_passant: m.is_en_passant(),
        }
    }
}

/// Previous four turns by `side`, plus the current turn when that turn has
/// already been played. When it is `side`'s turn, the current turn has not
/// been played yet. (Rule 14.1.4.)
pub fn history_window(moves: &[RecordedMove], side: Color, turn: Color) -> Vec<RecordedMove> {
    let own: Vec<&RecordedMove> = moves.iter().filter(|m| m.color == side).collect();
    let count = if turn == side { 4 } else { 5 };
    own[own.len().saturating_sub(count)..].iter().map(|m| (*m).clone()).collect()
}

/// Everything a strategy needs to read one position for one side.
pub struct EvalContext<'a> {
    pub board: &'a Board,
    pub pos: Position,
    pub side: Color,
    pub moves: &'a [RecordedMove],
    pub window: Vec<RecordedMove>,
    /// Set by the six-move search, which needs only the foundation, progress and
    /// completion flags: each strategy then returns as soon as its flags are
    /// settled, without the participants, targets and evidence text. The flags
    /// are the same either way (`tests/strategies.rs` checks this).
    pub flags_only: bool,
}

impl<'a> EvalContext<'a> {
    pub fn new(board: &'a Board, side: Color, moves: &'a [RecordedMove]) -> EvalContext<'a> {
        EvalContext {
            board,
            pos: Position::read(board),
            side,
            moves,
            window: history_window(moves, side, board.turn()),
            flags_only: false,
        }
    }

    /// The same context for reading only the three flags of each strategy.
    pub fn flags_only(board: &'a Board, side: Color, moves: &'a [RecordedMove]) -> EvalContext<'a> {
        EvalContext { flags_only: true, ..EvalContext::new(board, side, moves) }
    }
}

/// Play a list of SAN moves from the starting position.
pub fn play_sans(sans: &[&str]) -> Result<(Board, Vec<RecordedMove>), String> {
    let mut board = Board::start();
    let mut moves = Vec::new();
    for san in sans {
        let m = board.parse_san(san).ok_or(format!("Illegal move {san}"))?;
        moves.push(RecordedMove::new(&board, &m));
        board.make(&m);
    }
    Ok((board, moves))
}

/// Legal moves for `side`. When it is not that side's turn, the turn is passed
/// so recognition can see the moves the side would have if the opponent passed.
///
/// chess.js throws here when the side to move is in check (a pass is illegal),
/// which crashed the TypeScript version. This returns no moves instead.
pub fn moves_for(board: &Board, side: Color) -> Vec<Move> {
    if board.turn() == side {
        return board.legal_moves();
    }
    match board.with_turn_passed() {
        Some(passed) => passed.legal_moves(),
        None => Vec::new(),
    }
}
