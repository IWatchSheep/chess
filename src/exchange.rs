//! Capture valuation (Step 11 and Step 14.3; port of `exchange.ts`).
//!
//! The initial capture is always played. After it, each side recaptures only
//! with its cheapest legal recapturing piece, and stops when continuing does not
//! strictly improve its own result (equal nets stop).

use crate::board::{square_name, Board, Color, Move, PieceKind, Square};
use crate::geometry::{material_value, KING_COMPARE, MATE_BONUS};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureValuation {
    /// Total value of opposing pieces taken by the original player.
    pub expected_gain: i64,
    /// Total value of the original player's pieces taken by the opponent.
    pub potential_loss: i64,
    pub net: i64,
    pub mate: bool,
    /// Recaptures actually played after the initial capture.
    pub recapture_sans: Vec<String>,
}

#[derive(Clone, Debug)]
struct Outcome {
    expected_gain: i64,
    potential_loss: i64,
    stopped: bool,
    sequence: Vec<String>,
}

impl Outcome {
    fn stop() -> Outcome {
        Outcome { expected_gain: 0, potential_loss: 0, stopped: true, sequence: Vec::new() }
    }
    fn net(&self) -> i64 {
        self.expected_gain - self.potential_loss
    }
}

fn prefer(maximize_original: bool, options: Vec<Outcome>) -> Outcome {
    let mut iter = options.into_iter();
    let mut best = iter.next().expect("at least one option");
    for candidate in iter {
        let delta = candidate.net() - best.net();
        let better = if maximize_original { delta > 0 } else { delta < 0 };
        // On a tie, stopping is preferred to trading more material.
        let tie_stop = delta == 0 && candidate.stopped && !best.stopped;
        if better || tie_stop {
            best = candidate;
        }
    }
    best
}

fn promotion_extra(m: &Move) -> i64 {
    match m.promotion {
        Some(p) if p != PieceKind::King => material_value(p) - material_value(PieceKind::Pawn),
        _ => 0,
    }
}

fn captured_material(board: &Board, m: &Move) -> i64 {
    if m.is_en_passant() {
        return material_value(PieceKind::Pawn);
    }
    match board.piece_at(m.to) {
        Some(victim) if victim.kind != PieceKind::King => material_value(victim.kind),
        _ => 0,
    }
}

/// Value of the piece making a capture. A promoting pawn still counts as a pawn.
fn standing_value(m: &Move) -> i64 {
    if m.piece == PieceKind::King {
        KING_COMPARE
    } else {
        material_value(m.piece)
    }
}

fn promotion_rank(p: Option<PieceKind>) -> i32 {
    match p {
        Some(PieceKind::Queen) => 4,
        Some(PieceKind::Rook) => 3,
        Some(PieceKind::Bishop) => 2,
        Some(PieceKind::Knight) => 1,
        _ => 0,
    }
}

fn recaptures_on(board: &Board, sq: Square) -> Vec<Move> {
    let mut moves: Vec<Move> = board
        .legal_moves()
        .into_iter()
        .filter(|m| m.is_capture() && m.to == sq)
        .collect();
    moves.sort_by(|a, b| {
        square_name(a.from)
            .cmp(&square_name(b.from))
            .then(promotion_rank(b.promotion).cmp(&promotion_rank(a.promotion)))
    });
    moves
}

fn exchange(board: &Board, sq: Square, original: Color) -> Outcome {
    if board.is_game_over() {
        return Outcome::stop();
    }
    let side = board.turn();
    let moves = recaptures_on(board, sq);
    if moves.is_empty() {
        return Outcome::stop();
    }
    let cheapest = moves.iter().map(standing_value).min().unwrap();
    let maximize = side == original;
    let mut considered = vec![Outcome::stop()];

    for m in moves.iter().filter(|m| standing_value(m) == cheapest) {
        let taken = captured_material(board, m) + promotion_extra(m);
        let san = board.san(m);
        let next = board.play(m);

        if next.is_checkmate() {
            considered.push(if side == original {
                Outcome { expected_gain: taken + MATE_BONUS, potential_loss: 0, stopped: false, sequence: vec![san] }
            } else {
                Outcome { expected_gain: 0, potential_loss: taken + MATE_BONUS, stopped: false, sequence: vec![san] }
            });
            continue;
        }

        let cont = exchange(&next, sq, original);
        let mut sequence = vec![san];
        sequence.extend(cont.sequence.iter().cloned());
        considered.push(if side == original {
            Outcome {
                expected_gain: taken + cont.expected_gain,
                potential_loss: cont.potential_loss,
                stopped: false,
                sequence,
            }
        } else {
            Outcome {
                expected_gain: cont.expected_gain,
                potential_loss: taken + cont.potential_loss,
                stopped: false,
                sequence,
            }
        });
    }
    prefer(maximize, considered)
}

/// Price one legal move (normally a capture). Promotion adds the promoted
/// piece's value minus a pawn.
pub fn value_capture(board: &Board, m: &Move) -> CaptureValuation {
    let original = m.color;
    let initial = captured_material(board, m) + promotion_extra(m);
    let next = board.play(m);

    if next.is_checkmate() {
        let gain = initial + MATE_BONUS;
        return CaptureValuation { expected_gain: gain, potential_loss: 0, net: gain, mate: true, recapture_sans: vec![] };
    }

    let cont = exchange(&next, m.to, original);
    let expected_gain = initial + cont.expected_gain;
    let potential_loss = cont.potential_loss;
    CaptureValuation {
        expected_gain,
        potential_loss,
        net: expected_gain - potential_loss,
        mate: expected_gain >= MATE_BONUS || potential_loss >= MATE_BONUS,
        recapture_sans: cont.sequence,
    }
}
