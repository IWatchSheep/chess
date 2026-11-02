//! Piece values and geometric attack helpers (port of `geometry.ts`).
//!
//! Attacks here are geometric: pins and turn order are ignored, so a piece
//! with no legal move can still attack a square and still count toward a
//! strategy (Step 2 and rule 14.1.2).

use crate::board::{file_of, rank_of, square, square_name, Board, Color, PieceKind, Square};

/// A piece on a square.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Unit {
    pub square: Square,
    pub kind: PieceKind,
    pub color: Color,
}

impl Unit {
    pub fn name(&self) -> String {
        format!("{} {} on {}", self.color.name(), self.kind.name(), square_name(self.square))
    }
}

/// Centipawn piece values (Step 14.3).
pub fn material_value(kind: PieceKind) -> i64 {
    match kind {
        PieceKind::Pawn => 100,
        PieceKind::Knight => 320,
        PieceKind::Bishop => 330,
        PieceKind::Rook => 500,
        PieceKind::Queen => 900,
        PieceKind::King => 0,
    }
}

/// Used only to compare kings with other pieces inside fork and skewer rules.
pub const KING_COMPARE: i64 = 10_000;

/// Added to expected gain, or to potential loss, when a capture is checkmate.
pub const MATE_BONUS: i64 = 100_000;

pub fn compare_value(kind: PieceKind) -> i64 {
    if kind == PieceKind::King {
        KING_COMPARE
    } else {
        material_value(kind)
    }
}

/// 0 on the player's own back rank, 7 on the promotion rank.
pub fn relative_rank(color: Color, sq: Square) -> i32 {
    match color {
        Color::White => rank_of(sq),
        Color::Black => 7 - rank_of(sq),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shade {
    Light,
    Dark,
}

pub fn square_shade(sq: Square) -> Shade {
    if (file_of(sq) + rank_of(sq)) % 2 == 0 {
        Shade::Dark
    } else {
        Shade::Light
    }
}

pub fn uniq_units(units: Vec<Unit>) -> Vec<Unit> {
    let mut out: Vec<Unit> = Vec::with_capacity(units.len());
    for unit in units {
        if !out.contains(&unit) {
            out.push(unit);
        }
    }
    out
}

const KNIGHT_DELTAS: [(i32, i32); 8] =
    [(1, 2), (2, 1), (-1, 2), (-2, 1), (1, -2), (2, -1), (-1, -2), (-2, -1)];
const KING_DELTAS: [(i32, i32); 8] =
    [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];
const BISHOP_DIRS: [(i32, i32); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
const ROOK_DIRS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
const QUEEN_DIRS: [(i32, i32); 8] =
    [(1, 1), (1, -1), (-1, 1), (-1, -1), (1, 0), (-1, 0), (0, 1), (0, -1)];

pub fn slider_dirs(kind: PieceKind) -> &'static [(i32, i32)] {
    match kind {
        PieceKind::Bishop => &BISHOP_DIRS,
        PieceKind::Rook => &ROOK_DIRS,
        PieceKind::Queen => &QUEEN_DIRS,
        _ => &[],
    }
}

/// A read-only snapshot of where the pieces stand.
#[derive(Clone, Debug)]
pub struct Position {
    /// Pieces in board order (rank 8 to rank 1, file a to h).
    pub units: Vec<Unit>,
    grid: [Option<Unit>; 64],
}

impl Position {
    pub fn read(board: &Board) -> Position {
        let mut grid = [None; 64];
        let units: Vec<Unit> = board
            .pieces_in_board_order()
            .into_iter()
            .map(|(sq, p)| Unit { square: sq, kind: p.kind, color: p.color })
            .collect();
        for u in &units {
            grid[u.square as usize] = Some(*u);
        }
        Position { units, grid }
    }

    pub fn at(&self, sq: Square) -> Option<Unit> {
        self.grid[sq as usize]
    }

    pub fn of(&self, color: Color) -> Vec<Unit> {
        self.units.iter().filter(|u| u.color == color).copied().collect()
    }

    pub fn of_kind(&self, color: Color, kind: PieceKind) -> Vec<Unit> {
        self.units.iter().filter(|u| u.color == color && u.kind == kind).copied().collect()
    }

    pub fn king(&self, color: Color) -> Option<Unit> {
        self.units.iter().find(|u| u.color == color && u.kind == PieceKind::King).copied()
    }
}

/// Squares a piece of `kind`/`color` standing on `from` attacks geometrically.
pub fn attacked_squares(pos: &Position, from: Square, kind: PieceKind, color: Color) -> Vec<Square> {
    attacked_squares_ignoring(pos, from, kind, color, None)
}

/// Like [`attacked_squares`], but `vacated` is treated as empty. Used to look at
/// a piece's view from a destination square after it has left its origin.
pub fn attacked_squares_ignoring(pos: &Position, from: Square, kind: PieceKind, color: Color, vacated: Option<Square>) -> Vec<Square> {
    let file = file_of(from);
    let rank = rank_of(from);
    let mut out = Vec::new();
    match kind {
        PieceKind::Pawn => {
            let dir = color.forward();
            for df in [-1, 1] {
                if let Some(sq) = square(file + df, rank + dir) {
                    out.push(sq);
                }
            }
        }
        PieceKind::Knight | PieceKind::King => {
            let deltas = if kind == PieceKind::Knight { &KNIGHT_DELTAS } else { &KING_DELTAS };
            for (df, dr) in deltas {
                if let Some(sq) = square(file + df, rank + dr) {
                    out.push(sq);
                }
            }
        }
        _ => {
            for &(df, dr) in slider_dirs(kind) {
                let (mut f, mut r) = (file + df, rank + dr);
                while let Some(sq) = square(f, r) {
                    out.push(sq);
                    if Some(sq) != vacated && pos.at(sq).is_some() {
                        break;
                    }
                    f += df;
                    r += dr;
                }
            }
        }
    }
    out
}

pub fn attacks_square(pos: &Position, unit: &Unit, target: Square) -> bool {
    attacked_squares(pos, unit.square, unit.kind, unit.color).contains(&target)
}

/// Pieces of `color` that geometrically attack `target`.
pub fn units_attacking(pos: &Position, target: Square, color: Color) -> Vec<Unit> {
    pos.units
        .iter()
        .filter(|u| u.color == color && attacks_square(pos, u, target))
        .copied()
        .collect()
}

pub fn enemy_units_attacked_from(pos: &Position, from: Square, kind: PieceKind, color: Color) -> Vec<Unit> {
    let enemy = color.other();
    attacked_squares(pos, from, kind, color)
        .into_iter()
        .filter_map(|sq| pos.at(sq))
        .filter(|u| u.color == enemy)
        .collect()
}

pub fn enemy_units_attacked_by(pos: &Position, unit: &Unit) -> Vec<Unit> {
    enemy_units_attacked_from(pos, unit.square, unit.kind, unit.color)
}

/// Pieces met walking from `from` along a direction, nearest first.
pub fn ray_units(pos: &Position, from: Square, df: i32, dr: i32) -> Vec<Unit> {
    let mut out = Vec::new();
    let (mut f, mut r) = (file_of(from) + df, rank_of(from) + dr);
    while let Some(sq) = square(f, r) {
        if let Some(u) = pos.at(sq) {
            out.push(u);
        }
        f += df;
        r += dr;
    }
    out
}

/// Pieces strictly between two squares on a shared line, or `None` when the
/// squares are equal or not on a rank, file or diagonal.
pub fn piece_count_between(pos: Option<&Position>, a: Square, b: Square) -> Option<usize> {
    let df = file_of(b) - file_of(a);
    let dr = rank_of(b) - rank_of(a);
    if df == 0 && dr == 0 {
        return None;
    }
    if !(df == 0 || dr == 0 || df.abs() == dr.abs()) {
        return None;
    }
    let (sf, sr) = (df.signum(), dr.signum());
    let mut count = 0;
    let (mut f, mut r) = (file_of(a) + sf, rank_of(a) + sr);
    while f != file_of(b) || r != rank_of(b) {
        let sq = square(f, r)?;
        if pos.and_then(|p| p.at(sq)).is_some() {
            count += 1;
        }
        f += sf;
        r += sr;
    }
    Some(count)
}

pub fn pawns_on_file(pos: &Position, file: i32, color: Option<Color>) -> Vec<Unit> {
    pos.units
        .iter()
        .filter(|u| u.kind == PieceKind::Pawn && file_of(u.square) == file && color.is_none_or(|c| u.color == c))
        .copied()
        .collect()
}

/// Longest run of empty squares along any of the piece's slider directions.
pub fn clear_travel(pos: &Position, unit: &Unit) -> usize {
    let mut best = 0;
    for &(df, dr) in slider_dirs(unit.kind) {
        let mut count = 0;
        let (mut f, mut r) = (file_of(unit.square) + df, rank_of(unit.square) + dr);
        while let Some(sq) = square(f, r) {
            if pos.at(sq).is_some() {
                break;
            }
            count += 1;
            f += df;
            r += dr;
        }
        best = best.max(count);
    }
    best
}

pub fn is_passed_pawn(pos: &Position, pawn: &Unit) -> bool {
    if pawn.kind != PieceKind::Pawn {
        return false;
    }
    let rel = relative_rank(pawn.color, pawn.square);
    let file = file_of(pawn.square);
    !pos.of_kind(pawn.color.other(), PieceKind::Pawn).iter().any(|other| {
        (file_of(other.square) - file).abs() <= 1 && relative_rank(pawn.color, other.square) > rel
    })
}

pub fn is_isolated_pawn(pos: &Position, pawn: &Unit) -> bool {
    if pawn.kind != PieceKind::Pawn {
        return false;
    }
    let file = file_of(pawn.square);
    !pos.of_kind(pawn.color, PieceKind::Pawn)
        .iter()
        .any(|other| (file_of(other.square) - file).abs() == 1)
}

pub fn join_squares(units: &[Unit], sep: &str) -> String {
    units.iter().map(|u| square_name(u.square)).collect::<Vec<_>>().join(sep)
}
