//! Board representation, legal move generation, FEN and SAN.
//!
//! This replaces `chess.js` from the TypeScript version. It has no external
//! dependencies. Correctness is checked against the standard move counts from
//! Step 6 (20, 400, 8,902, 197,281, 4,865,609, 119,060,324) in `tests/perft.rs`.

use std::fmt;

// ---------------------------------------------------------------------------
// Colors, pieces, squares
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Color {
    White,
    Black,
}

impl Color {
    pub const BOTH: [Color; 2] = [Color::White, Color::Black];

    pub fn other(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
    pub fn index(self) -> usize {
        self as usize
    }
    pub fn name(self) -> &'static str {
        match self {
            Color::White => "White",
            Color::Black => "Black",
        }
    }
    pub fn letter(self) -> char {
        match self {
            Color::White => 'w',
            Color::Black => 'b',
        }
    }
    pub fn from_letter(c: char) -> Option<Color> {
        match c.to_ascii_lowercase() {
            'w' => Some(Color::White),
            'b' => Some(Color::Black),
            _ => None,
        }
    }
    /// +1 for White (up the board), -1 for Black.
    pub fn forward(self) -> i32 {
        match self {
            Color::White => 1,
            Color::Black => -1,
        }
    }
    /// Rank index (0-7) of this side's back rank.
    pub fn home_rank(self) -> i32 {
        match self {
            Color::White => 0,
            Color::Black => 7,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum PieceKind {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

impl PieceKind {
    pub const ALL: [PieceKind; 6] = [
        PieceKind::Pawn,
        PieceKind::Knight,
        PieceKind::Bishop,
        PieceKind::Rook,
        PieceKind::Queen,
        PieceKind::King,
    ];
    pub const PROMOTIONS: [PieceKind; 4] =
        [PieceKind::Queen, PieceKind::Rook, PieceKind::Bishop, PieceKind::Knight];

    pub fn index(self) -> usize {
        self as usize
    }
    pub fn name(self) -> &'static str {
        match self {
            PieceKind::Pawn => "pawn",
            PieceKind::Knight => "knight",
            PieceKind::Bishop => "bishop",
            PieceKind::Rook => "rook",
            PieceKind::Queen => "queen",
            PieceKind::King => "king",
        }
    }
    /// Upper-case letter used in SAN and FEN (P, N, B, R, Q, K).
    pub fn letter(self) -> char {
        match self {
            PieceKind::Pawn => 'P',
            PieceKind::Knight => 'N',
            PieceKind::Bishop => 'B',
            PieceKind::Rook => 'R',
            PieceKind::Queen => 'Q',
            PieceKind::King => 'K',
        }
    }
    pub fn from_letter(c: char) -> Option<PieceKind> {
        match c.to_ascii_uppercase() {
            'P' => Some(PieceKind::Pawn),
            'N' => Some(PieceKind::Knight),
            'B' => Some(PieceKind::Bishop),
            'R' => Some(PieceKind::Rook),
            'Q' => Some(PieceKind::Queen),
            'K' => Some(PieceKind::King),
            _ => None,
        }
    }
    pub fn is_slider(self) -> bool {
        matches!(self, PieceKind::Bishop | PieceKind::Rook | PieceKind::Queen)
    }
    pub fn is_major(self) -> bool {
        matches!(self, PieceKind::Rook | PieceKind::Queen)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Piece {
    pub color: Color,
    pub kind: PieceKind,
}

impl Piece {
    pub fn new(color: Color, kind: PieceKind) -> Piece {
        Piece { color, kind }
    }
    /// FEN character: upper case for White, lower case for Black.
    pub fn fen_char(self) -> char {
        let c = self.kind.letter();
        match self.color {
            Color::White => c,
            Color::Black => c.to_ascii_lowercase(),
        }
    }
}

/// Square index: 0 = a1, 7 = h1, 56 = a8, 63 = h8.
pub type Square = u8;

pub fn square(file: i32, rank: i32) -> Option<Square> {
    if (0..8).contains(&file) && (0..8).contains(&rank) {
        Some((rank * 8 + file) as Square)
    } else {
        None
    }
}
pub fn file_of(sq: Square) -> i32 {
    (sq % 8) as i32
}
pub fn rank_of(sq: Square) -> i32 {
    (sq / 8) as i32
}
pub fn square_name(sq: Square) -> String {
    let mut s = String::with_capacity(2);
    s.push((b'a' + sq % 8) as char);
    s.push((b'1' + sq / 8) as char);
    s
}
pub fn parse_square(text: &str) -> Option<Square> {
    let bytes = text.trim().as_bytes();
    if bytes.len() != 2 {
        return None;
    }
    let file = bytes[0].to_ascii_lowercase();
    let rank = bytes[1];
    if !(b'a'..=b'h').contains(&file) || !(b'1'..=b'8').contains(&rank) {
        return None;
    }
    square((file - b'a') as i32, (rank - b'1') as i32)
}

// ---------------------------------------------------------------------------
// Moves
// ---------------------------------------------------------------------------

pub const FLAG_EN_PASSANT: u8 = 1;
pub const FLAG_CASTLE_KING: u8 = 2;
pub const FLAG_CASTLE_QUEEN: u8 = 4;
pub const FLAG_DOUBLE_PUSH: u8 = 8;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Move {
    pub color: Color,
    pub from: Square,
    pub to: Square,
    pub piece: PieceKind,
    /// The captured piece. For en passant this is `Pawn`.
    pub captured: Option<PieceKind>,
    pub promotion: Option<PieceKind>,
    pub flags: u8,
}

impl Move {
    pub fn is_capture(&self) -> bool {
        self.captured.is_some()
    }
    pub fn is_en_passant(&self) -> bool {
        self.flags & FLAG_EN_PASSANT != 0
    }
    pub fn is_castle(&self) -> bool {
        self.flags & (FLAG_CASTLE_KING | FLAG_CASTLE_QUEEN) != 0
    }
    /// Square of the captured piece (differs from `to` only for en passant).
    pub fn victim_square(&self) -> Square {
        if self.is_en_passant() {
            square(file_of(self.to), rank_of(self.from)).unwrap()
        } else {
            self.to
        }
    }
    /// Coordinate notation such as `e2e4` or `e7e8q`.
    pub fn uci(&self) -> String {
        let mut s = square_name(self.from);
        s.push_str(&square_name(self.to));
        if let Some(p) = self.promotion {
            s.push(p.letter().to_ascii_lowercase());
        }
        s
    }
}

// ---------------------------------------------------------------------------
// Attack tables
// ---------------------------------------------------------------------------

const fn gen_leaper(deltas: &[(i32, i32)]) -> [u64; 64] {
    let mut table = [0u64; 64];
    let mut sq = 0;
    while sq < 64 {
        let file = (sq % 8) as i32;
        let rank = (sq / 8) as i32;
        let mut i = 0;
        let mut bits = 0u64;
        while i < deltas.len() {
            let f = file + deltas[i].0;
            let r = rank + deltas[i].1;
            if f >= 0 && f < 8 && r >= 0 && r < 8 {
                bits |= 1u64 << (r * 8 + f);
            }
            i += 1;
        }
        table[sq] = bits;
        sq += 1;
    }
    table
}

const KNIGHT_DELTAS: [(i32, i32); 8] =
    [(1, 2), (2, 1), (-1, 2), (-2, 1), (1, -2), (2, -1), (-1, -2), (-2, -1)];
const KING_DELTAS: [(i32, i32); 8] =
    [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];

const KNIGHT_ATTACKS: [u64; 64] = gen_leaper(&KNIGHT_DELTAS);
const KING_ATTACKS: [u64; 64] = gen_leaper(&KING_DELTAS);
const PAWN_ATTACKS: [[u64; 64]; 2] = [gen_leaper(&[(-1, 1), (1, 1)]), gen_leaper(&[(-1, -1), (1, -1)])];

/// Ray directions: N, NE, E, SE, S, SW, W, NW.
const DIRS: [(i32, i32); 8] = [(0, 1), (1, 1), (1, 0), (1, -1), (0, -1), (-1, -1), (-1, 0), (-1, 1)];
/// Whether each direction increases the square index.
const DIR_POSITIVE: [bool; 8] = [true, true, true, false, false, false, false, true];
const ROOK_DIR_IDX: [usize; 4] = [0, 2, 4, 6];
const BISHOP_DIR_IDX: [usize; 4] = [1, 3, 5, 7];

const fn gen_rays() -> [[u64; 64]; 8] {
    let mut rays = [[0u64; 64]; 8];
    let mut d = 0;
    while d < 8 {
        let mut sq = 0;
        while sq < 64 {
            let mut f = (sq % 8) as i32 + DIRS[d].0;
            let mut r = (sq / 8) as i32 + DIRS[d].1;
            let mut bits = 0u64;
            while f >= 0 && f < 8 && r >= 0 && r < 8 {
                bits |= 1u64 << (r * 8 + f);
                f += DIRS[d].0;
                r += DIRS[d].1;
            }
            rays[d][sq] = bits;
            sq += 1;
        }
        d += 1;
    }
    rays
}

const RAYS: [[u64; 64]; 8] = gen_rays();

#[inline]
fn ray_attacks(dir: usize, sq: Square, occupied: u64) -> u64 {
    let ray = RAYS[dir][sq as usize];
    let blockers = ray & occupied;
    if blockers == 0 {
        return ray;
    }
    let first = if DIR_POSITIVE[dir] {
        blockers.trailing_zeros()
    } else {
        63 - blockers.leading_zeros()
    };
    ray ^ RAYS[dir][first as usize]
}

#[inline]
fn rook_attacks(sq: Square, occupied: u64) -> u64 {
    ROOK_DIR_IDX.iter().fold(0, |acc, &d| acc | ray_attacks(d, sq, occupied))
}

#[inline]
fn bishop_attacks(sq: Square, occupied: u64) -> u64 {
    BISHOP_DIR_IDX.iter().fold(0, |acc, &d| acc | ray_attacks(d, sq, occupied))
}

#[inline]
fn bit(sq: Square) -> u64 {
    1u64 << sq
}

/// Iterate over the set bits of a bitboard as squares.
fn squares_of(mut bb: u64) -> impl Iterator<Item = Square> {
    std::iter::from_fn(move || {
        if bb == 0 {
            None
        } else {
            let sq = bb.trailing_zeros() as Square;
            bb &= bb - 1;
            Some(sq)
        }
    })
}

// ---------------------------------------------------------------------------
// Board
// ---------------------------------------------------------------------------

pub const CASTLE_WK: u8 = 1;
pub const CASTLE_WQ: u8 = 2;
pub const CASTLE_BK: u8 = 4;
pub const CASTLE_BQ: u8 = 8;

pub const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Board {
    by_kind: [u64; 6],
    by_color: [u64; 2],
    squares: [Option<Piece>; 64],
    turn: Color,
    castling: u8,
    ep: Option<Square>,
    halfmove: u32,
    fullmove: u32,
}

/// Castling rights for one side.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CastlingRights {
    pub king_side: bool,
    pub queen_side: bool,
}

impl Default for Board {
    fn default() -> Self {
        Board::start()
    }
}

/// A position without its move clocks: pieces, side to move, castling rights
/// and en-passant square. Legal moves, and every reading of the 19 strategies
/// that do not look at the move history, are the same for two boards with the
/// same key (no strategy reads the halfmove or fullmove counters).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BoardKey {
    by_kind: [u64; 6],
    by_color: [u64; 2],
    turn: Color,
    castling: u8,
    ep: Option<Square>,
}

impl Board {
    pub fn key(&self) -> BoardKey {
        BoardKey { by_kind: self.by_kind, by_color: self.by_color, turn: self.turn, castling: self.castling, ep: self.ep }
    }

    /// The same information as [`key`](Self::key) in 32 bytes: the occupied
    /// squares, then one 4-bit code (color and kind) per piece in square order,
    /// then side to move, castling rights and en-passant square. Two boards have
    /// the same packed key exactly when they have the same `key`. `None` for a
    /// board with more than 32 pieces (no legal game has one).
    pub fn packed_key(&self) -> Option<[u64; 4]> {
        let occupied = self.by_color[0] | self.by_color[1];
        if occupied.count_ones() > 32 {
            return None;
        }
        let mut codes = [0u64; 2];
        let mut bits = occupied;
        let mut n = 0;
        while bits != 0 {
            let sq = bits.trailing_zeros() as usize;
            bits &= bits - 1;
            let p = self.squares[sq].expect("occupied square has a piece");
            let code = (p.color.index() as u64) << 3 | (p.kind.index() as u64 + 1);
            codes[n / 16] |= code << (4 * (n % 16));
            n += 1;
        }
        let ep = self.ep.map(|s| s as u64 + 1).unwrap_or(0);
        let tail = (self.turn.index() as u64) | (self.castling as u64) << 1 | ep << 9;
        Some([occupied, codes[0], codes[1], tail])
    }

    fn empty() -> Board {
        Board {
            by_kind: [0; 6],
            by_color: [0; 2],
            squares: [None; 64],
            turn: Color::White,
            castling: 0,
            ep: None,
            halfmove: 0,
            fullmove: 1,
        }
    }

    pub fn start() -> Board {
        Board::from_fen(START_FEN).expect("start position")
    }

    // ----- FEN -----------------------------------------------------------

    pub fn from_fen(fen: &str) -> Result<Board, String> {
        let fields: Vec<&str> = fen.split_whitespace().collect();
        if fields.len() < 4 {
            return Err("A FEN needs at least four fields.".into());
        }
        let mut board = Board::empty();
        let rows: Vec<&str> = fields[0].split('/').collect();
        if rows.len() != 8 {
            return Err("The board field must have eight ranks.".into());
        }
        for (i, row) in rows.iter().enumerate() {
            let rank = 7 - i as i32;
            let mut file = 0;
            for c in row.chars() {
                if let Some(n) = c.to_digit(10) {
                    file += n as i32;
                } else {
                    let kind = PieceKind::from_letter(c).ok_or(format!("Unknown piece '{c}'."))?;
                    let color = if c.is_ascii_uppercase() { Color::White } else { Color::Black };
                    let sq = square(file, rank).ok_or("A rank has too many squares.")?;
                    board.put(sq, Piece::new(color, kind));
                    file += 1;
                }
            }
            if file != 8 {
                return Err(format!("Rank {} does not have eight squares.", rank + 1));
            }
        }
        for color in Color::BOTH {
            if (board.by_kind[PieceKind::King.index()] & board.by_color[color.index()]).count_ones() != 1 {
                return Err(format!("{} must have exactly one king.", color.name()));
            }
        }
        board.turn = match fields[1] {
            "w" => Color::White,
            "b" => Color::Black,
            other => return Err(format!("Unknown side to move '{other}'.")),
        };
        if fields[2] != "-" {
            for c in fields[2].chars() {
                board.castling |= match c {
                    'K' => CASTLE_WK,
                    'Q' => CASTLE_WQ,
                    'k' => CASTLE_BK,
                    'q' => CASTLE_BQ,
                    _ => return Err(format!("Unknown castling flag '{c}'.")),
                };
            }
        }
        // Drop rights whose king or rook is not on its original square.
        let keep = |b: &Board, k: Square, r: Square, color: Color| {
            b.piece_at(k) == Some(Piece::new(color, PieceKind::King))
                && b.piece_at(r) == Some(Piece::new(color, PieceKind::Rook))
        };
        if !keep(&board, 4, 7, Color::White) {
            board.castling &= !CASTLE_WK;
        }
        if !keep(&board, 4, 0, Color::White) {
            board.castling &= !CASTLE_WQ;
        }
        if !keep(&board, 60, 63, Color::Black) {
            board.castling &= !CASTLE_BK;
        }
        if !keep(&board, 60, 56, Color::Black) {
            board.castling &= !CASTLE_BQ;
        }
        let pawns = board.by_kind[PieceKind::Pawn.index()];
        if pawns & 0xFF00_0000_0000_00FF != 0 {
            return Err("Pawns cannot stand on the first or eighth rank.".into());
        }
        let (wk, bk) = (board.king_square(Color::White), board.king_square(Color::Black));
        if KING_ATTACKS[wk as usize] & bit(bk) != 0 {
            return Err("The two kings cannot stand next to each other.".into());
        }
        board.ep = if fields[3] == "-" {
            None
        } else {
            let ep = parse_square(fields[3]).ok_or(format!("Bad en passant square '{}'.", fields[3]))?;
            // The square behind a pawn that has just made a double step: it must be empty,
            // the pawn must be in front of it, and the square the pawn came from must be empty.
            let mover = board.turn.other();
            let (ep_rank, pawn_rank, from_rank) = if mover == Color::White { (2, 3, 1) } else { (5, 4, 6) };
            let pawn_sq = square(file_of(ep), pawn_rank);
            let from_sq = square(file_of(ep), from_rank);
            let valid = rank_of(ep) == ep_rank
                && board.squares[ep as usize].is_none()
                && pawn_sq.is_some_and(|s| board.squares[s as usize] == Some(Piece::new(mover, PieceKind::Pawn)))
                && from_sq.is_some_and(|s| board.squares[s as usize].is_none());
            if !valid {
                return Err(format!("The en passant square '{}' does not match the position.", fields[3]));
            }
            Some(ep)
        };
        board.halfmove = fields.get(4).and_then(|s| s.parse().ok()).unwrap_or(0);
        board.fullmove = fields.get(5).and_then(|s| s.parse().ok()).unwrap_or(1).max(1);
        // Like chess.js, a position where the side not to move is in check is
        // accepted (the skewer reference position uses one). King captures are
        // never generated, so such a position is still handled safely.
        Ok(board)
    }

    /// FEN string. Like chess.js, the en passant square is only written when a
    /// pawn is actually able to capture there.
    pub fn fen(&self) -> String {
        let mut out = self.placement_fen();
        out.push(' ');
        out.push(self.turn.letter());
        out.push(' ');
        out.push_str(&self.castling_fen());
        out.push(' ');
        out.push_str(&self.ep_fen());
        out.push_str(&format!(" {} {}", self.halfmove, self.fullmove));
        out
    }

    /// FEN without the move counters. Used for repetition detection.
    pub fn position_key(&self) -> String {
        format!(
            "{} {} {} {}",
            self.placement_fen(),
            self.turn.letter(),
            self.castling_fen(),
            self.ep_fen()
        )
    }

    fn placement_fen(&self) -> String {
        let mut out = String::new();
        for rank in (0..8).rev() {
            let mut empty = 0;
            for file in 0..8 {
                match self.squares[(rank * 8 + file) as usize] {
                    Some(p) => {
                        if empty > 0 {
                            out.push_str(&empty.to_string());
                            empty = 0;
                        }
                        out.push(p.fen_char());
                    }
                    None => empty += 1,
                }
            }
            if empty > 0 {
                out.push_str(&empty.to_string());
            }
            if rank > 0 {
                out.push('/');
            }
        }
        out
    }

    fn castling_fen(&self) -> String {
        let mut s = String::new();
        for (flag, c) in [(CASTLE_WK, 'K'), (CASTLE_WQ, 'Q'), (CASTLE_BK, 'k'), (CASTLE_BQ, 'q')] {
            if self.castling & flag != 0 {
                s.push(c);
            }
        }
        if s.is_empty() {
            s.push('-');
        }
        s
    }

    fn ep_fen(&self) -> String {
        match self.ep {
            Some(ep) if self.ep_capturable(ep) => square_name(ep),
            _ => "-".into(),
        }
    }

    fn ep_capturable(&self, ep: Square) -> bool {
        let us = self.turn;
        PAWN_ATTACKS[us.other().index()][ep as usize] & self.pieces(us, PieceKind::Pawn) != 0
    }

    // ----- Accessors -----------------------------------------------------

    pub fn turn(&self) -> Color {
        self.turn
    }
    pub fn piece_at(&self, sq: Square) -> Option<Piece> {
        self.squares[sq as usize]
    }
    pub fn halfmove_clock(&self) -> u32 {
        self.halfmove
    }
    pub fn fullmove_number(&self) -> u32 {
        self.fullmove
    }
    pub fn castling_rights(&self, color: Color) -> CastlingRights {
        match color {
            Color::White => CastlingRights {
                king_side: self.castling & CASTLE_WK != 0,
                queen_side: self.castling & CASTLE_WQ != 0,
            },
            Color::Black => CastlingRights {
                king_side: self.castling & CASTLE_BK != 0,
                queen_side: self.castling & CASTLE_BQ != 0,
            },
        }
    }
    fn pieces(&self, color: Color, kind: PieceKind) -> u64 {
        self.by_kind[kind.index()] & self.by_color[color.index()]
    }
    fn occupied(&self) -> u64 {
        self.by_color[0] | self.by_color[1]
    }
    pub fn king_square(&self, color: Color) -> Square {
        self.pieces(color, PieceKind::King).trailing_zeros() as Square
    }

    /// All pieces in board order: rank 8 down to rank 1, file a to h.
    /// (This is the order chess.js `board()` uses.)
    pub fn pieces_in_board_order(&self) -> Vec<(Square, Piece)> {
        let mut out = Vec::with_capacity(32);
        for rank in (0..8).rev() {
            for file in 0..8 {
                let sq = (rank * 8 + file) as Square;
                if let Some(p) = self.squares[sq as usize] {
                    out.push((sq, p));
                }
            }
        }
        out
    }

    fn put(&mut self, sq: Square, piece: Piece) {
        self.squares[sq as usize] = Some(piece);
        self.by_kind[piece.kind.index()] |= bit(sq);
        self.by_color[piece.color.index()] |= bit(sq);
    }

    fn remove(&mut self, sq: Square) -> Option<Piece> {
        let piece = self.squares[sq as usize].take()?;
        self.by_kind[piece.kind.index()] &= !bit(sq);
        self.by_color[piece.color.index()] &= !bit(sq);
        Some(piece)
    }

    // ----- Attacks -------------------------------------------------------

    /// Squares attacked by `color` pieces that attack `sq`.
    pub fn attackers_to(&self, sq: Square, by: Color) -> u64 {
        let occ = self.occupied();
        let i = sq as usize;
        let queens = self.pieces(by, PieceKind::Queen);
        (PAWN_ATTACKS[by.other().index()][i] & self.pieces(by, PieceKind::Pawn))
            | (KNIGHT_ATTACKS[i] & self.pieces(by, PieceKind::Knight))
            | (KING_ATTACKS[i] & self.pieces(by, PieceKind::King))
            | (bishop_attacks(sq, occ) & (self.pieces(by, PieceKind::Bishop) | queens))
            | (rook_attacks(sq, occ) & (self.pieces(by, PieceKind::Rook) | queens))
    }

    pub fn is_square_attacked(&self, sq: Square, by: Color) -> bool {
        self.attackers_to(sq, by) != 0
    }

    pub fn in_check(&self, color: Color) -> bool {
        self.is_square_attacked(self.king_square(color), color.other())
    }

    /// Whether the side to move is in check.
    pub fn is_check(&self) -> bool {
        self.in_check(self.turn)
    }

    // ----- Move generation -----------------------------------------------

    fn push_pawn_move(out: &mut Vec<Move>, color: Color, from: Square, to: Square, captured: Option<PieceKind>, flags: u8) {
        let last = if color == Color::White { 7 } else { 0 };
        if rank_of(to) == last {
            for promo in PieceKind::PROMOTIONS {
                out.push(Move { color, from, to, piece: PieceKind::Pawn, captured, promotion: Some(promo), flags });
            }
        } else {
            out.push(Move { color, from, to, piece: PieceKind::Pawn, captured, promotion: None, flags });
        }
    }

    fn pseudo_moves(&self, out: &mut Vec<Move>) {
        let us = self.turn;
        let them = us.other();
        let own = self.by_color[us.index()];
        let enemy_all = self.by_color[them.index()];
        let occ = own | enemy_all;
        // Kings are never captured.
        let enemy = enemy_all & !self.by_kind[PieceKind::King.index()];

        // Pawns
        let dir = us.forward();
        let start_rank = if us == Color::White { 1 } else { 6 };
        for from in squares_of(self.pieces(us, PieceKind::Pawn)) {
            let f = file_of(from);
            let r = rank_of(from);
            if let Some(one) = square(f, r + dir) {
                if occ & bit(one) == 0 {
                    Self::push_pawn_move(out, us, from, one, None, 0);
                    if r == start_rank {
                        let two = square(f, r + 2 * dir).unwrap();
                        if occ & bit(two) == 0 {
                            out.push(Move { color: us, from, to: two, piece: PieceKind::Pawn, captured: None, promotion: None, flags: FLAG_DOUBLE_PUSH });
                        }
                    }
                }
            }
            let attacks = PAWN_ATTACKS[us.index()][from as usize];
            for to in squares_of(attacks & enemy) {
                let captured = self.squares[to as usize].map(|p| p.kind);
                Self::push_pawn_move(out, us, from, to, captured, 0);
            }
            if let Some(ep) = self.ep {
                if attacks & bit(ep) != 0 {
                    out.push(Move { color: us, from, to: ep, piece: PieceKind::Pawn, captured: Some(PieceKind::Pawn), promotion: None, flags: FLAG_EN_PASSANT });
                }
            }
        }

        // Pieces
        for kind in [PieceKind::Knight, PieceKind::Bishop, PieceKind::Rook, PieceKind::Queen, PieceKind::King] {
            for from in squares_of(self.pieces(us, kind)) {
                let targets = match kind {
                    PieceKind::Knight => KNIGHT_ATTACKS[from as usize],
                    PieceKind::Bishop => bishop_attacks(from, occ),
                    PieceKind::Rook => rook_attacks(from, occ),
                    PieceKind::Queen => bishop_attacks(from, occ) | rook_attacks(from, occ),
                    PieceKind::King => KING_ATTACKS[from as usize],
                    PieceKind::Pawn => unreachable!(),
                } & !own & !(enemy_all & !enemy);
                for to in squares_of(targets) {
                    let captured = self.squares[to as usize].map(|p| p.kind);
                    out.push(Move { color: us, from, to, piece: kind, captured, promotion: None, flags: 0 });
                }
            }
        }

        // Castling
        let rights = self.castling_rights(us);
        if rights.king_side || rights.queen_side {
            let base = if us == Color::White { 0u8 } else { 56u8 };
            let king = base + 4;
            if self.king_square(us) == king && !self.is_square_attacked(king, them) {
                if rights.king_side
                    && occ & (bit(base + 5) | bit(base + 6)) == 0
                    && !self.is_square_attacked(base + 5, them)
                    && !self.is_square_attacked(base + 6, them)
                {
                    out.push(Move { color: us, from: king, to: base + 6, piece: PieceKind::King, captured: None, promotion: None, flags: FLAG_CASTLE_KING });
                }
                if rights.queen_side
                    && occ & (bit(base + 1) | bit(base + 2) | bit(base + 3)) == 0
                    && !self.is_square_attacked(base + 3, them)
                    && !self.is_square_attacked(base + 2, them)
                {
                    out.push(Move { color: us, from: king, to: base + 2, piece: PieceKind::King, captured: None, promotion: None, flags: FLAG_CASTLE_QUEEN });
                }
            }
        }
    }

    /// Append the legal moves of the side to move to `out`.
    pub fn legal_moves_into(&self, out: &mut Vec<Move>) {
        let mut pseudo = Vec::with_capacity(64);
        self.pseudo_moves(&mut pseudo);
        let us = self.turn;
        for m in pseudo {
            let next = self.play(&m);
            if !next.in_check(us) {
                out.push(m);
            }
        }
    }

    /// Legal moves of the side to move.
    pub fn legal_moves(&self) -> Vec<Move> {
        let mut out = Vec::with_capacity(64);
        self.legal_moves_into(&mut out);
        out
    }

    /// Number of legal moves (no allocation of the result list).
    pub fn count_legal_moves(&self) -> u64 {
        let mut pseudo = Vec::with_capacity(64);
        self.pseudo_moves(&mut pseudo);
        let us = self.turn;
        pseudo.iter().filter(|m| !self.play(m).in_check(us)).count() as u64
    }

    pub fn has_legal_move(&self) -> bool {
        let mut pseudo = Vec::with_capacity(64);
        self.pseudo_moves(&mut pseudo);
        let us = self.turn;
        pseudo.iter().any(|m| !self.play(m).in_check(us))
    }

    // ----- Making moves --------------------------------------------------

    /// Apply a move in place. The move must be legal (or at least pseudo-legal)
    /// for this position.
    pub fn make(&mut self, m: &Move) {
        let us = self.turn;
        if m.piece == PieceKind::Pawn || m.captured.is_some() {
            self.halfmove = 0;
        } else {
            self.halfmove += 1;
        }
        if m.is_en_passant() {
            self.remove(m.victim_square());
        } else if m.captured.is_some() {
            self.remove(m.to);
        }
        let moving = self.remove(m.from).expect("moving piece");
        let placed = match m.promotion {
            Some(kind) => Piece::new(us, kind),
            None => moving,
        };
        self.put(m.to, placed);

        if m.flags & FLAG_CASTLE_KING != 0 {
            let rook = self.remove(m.to + 1).expect("castling rook");
            self.put(m.to - 1, rook);
        } else if m.flags & FLAG_CASTLE_QUEEN != 0 {
            let rook = self.remove(m.to - 2).expect("castling rook");
            self.put(m.to + 1, rook);
        }

        if m.piece == PieceKind::King {
            self.castling &= match us {
                Color::White => !(CASTLE_WK | CASTLE_WQ),
                Color::Black => !(CASTLE_BK | CASTLE_BQ),
            };
        }
        for sq in [m.from, m.to] {
            self.castling &= match sq {
                0 => !CASTLE_WQ,
                7 => !CASTLE_WK,
                56 => !CASTLE_BQ,
                63 => !CASTLE_BK,
                _ => 0xFF,
            };
        }

        self.ep = if m.flags & FLAG_DOUBLE_PUSH != 0 {
            square(file_of(m.from), (rank_of(m.from) + rank_of(m.to)) / 2)
        } else {
            None
        };
        if us == Color::Black {
            self.fullmove += 1;
        }
        self.turn = us.other();
    }

    /// The position after a move.
    pub fn play(&self, m: &Move) -> Board {
        let mut next = *self;
        next.make(m);
        next
    }

    /// The position after the side to move passes (a "null move").
    /// Mirrors chess.js `setTurn`: returns `None` when the side to move is in
    /// check, because passing would leave its king capturable.
    pub fn with_turn_passed(&self) -> Option<Board> {
        if self.is_check() {
            return None;
        }
        let mut next = *self;
        if self.turn == Color::Black {
            next.fullmove += 1;
        }
        next.halfmove += 1;
        next.ep = None;
        next.turn = self.turn.other();
        Some(next)
    }

    /// Legal moves `color` would have if it were that side's turn, with no
    /// requirement that the opponent's king be safe. Used only for the
    /// "all pieces' moves" listings in the history files.
    pub fn moves_if_turn(&self, color: Color) -> Vec<Move> {
        self.with_turn(color).legal_moves()
    }

    /// The same position with `color` to move (en passant cleared if the turn
    /// changes). Unlike `with_turn_passed`, this works even when the side to
    /// move is in check; it is only for listing what pieces could do.
    pub fn with_turn(&self, color: Color) -> Board {
        let mut copy = *self;
        if copy.turn != color {
            copy.turn = color;
            copy.ep = None;
        }
        copy
    }

    // ----- Game state ----------------------------------------------------

    pub fn is_checkmate(&self) -> bool {
        self.is_check() && !self.has_legal_move()
    }

    pub fn is_stalemate(&self) -> bool {
        !self.is_check() && !self.has_legal_move()
    }

    /// Same rule as chess.js: K v K, K+minor v K, or only bishops all on one colour.
    pub fn is_insufficient_material(&self) -> bool {
        let all = self.occupied();
        let pieces = all.count_ones();
        if pieces == 2 {
            return true;
        }
        let bishops = self.by_kind[PieceKind::Bishop.index()];
        let knights = self.by_kind[PieceKind::Knight.index()];
        if pieces == 3 && (bishops.count_ones() == 1 || knights.count_ones() == 1) {
            return true;
        }
        if pieces == bishops.count_ones() + 2 {
            let dark = squares_of(bishops).filter(|&sq| (file_of(sq) + rank_of(sq)) % 2 == 0).count() as u32;
            if dark == 0 || dark == bishops.count_ones() {
                return true;
            }
        }
        false
    }

    pub fn is_fifty_move_draw(&self) -> bool {
        self.halfmove >= 100
    }

    /// Checkmate, stalemate, insufficient material or the fifty-move rule.
    /// (Repetition needs game history, which the game loop tracks separately.)
    pub fn is_game_over(&self) -> bool {
        self.is_fifty_move_draw() || self.is_insufficient_material() || !self.has_legal_move()
    }

    // ----- SAN -------------------------------------------------------------

    /// Standard Algebraic Notation for a legal move, using the same
    /// disambiguation rules as chess.js. `legal` must be this position's legal moves.
    pub fn san_with(&self, m: &Move, legal: &[Move]) -> String {
        let mut out = String::new();
        if m.flags & FLAG_CASTLE_KING != 0 {
            out.push_str("O-O");
        } else if m.flags & FLAG_CASTLE_QUEEN != 0 {
            out.push_str("O-O-O");
        } else {
            if m.piece != PieceKind::Pawn {
                out.push(m.piece.letter());
                let mut ambiguities = 0;
                let mut same_rank = 0;
                let mut same_file = 0;
                for other in legal {
                    if other.piece == m.piece && other.from != m.from && other.to == m.to {
                        ambiguities += 1;
                        if rank_of(other.from) == rank_of(m.from) {
                            same_rank += 1;
                        }
                        if file_of(other.from) == file_of(m.from) {
                            same_file += 1;
                        }
                    }
                }
                if ambiguities > 0 {
                    let name = square_name(m.from);
                    if same_rank > 0 && same_file > 0 {
                        out.push_str(&name);
                    } else if same_file > 0 {
                        out.push_str(&name[1..2]);
                    } else {
                        out.push_str(&name[0..1]);
                    }
                }
            }
            if m.is_capture() {
                if m.piece == PieceKind::Pawn {
                    out.push_str(&square_name(m.from)[0..1]);
                }
                out.push('x');
            }
            out.push_str(&square_name(m.to));
            if let Some(p) = m.promotion {
                out.push('=');
                out.push(p.letter());
            }
        }
        let next = self.play(m);
        if next.is_check() {
            out.push(if next.has_legal_move() { '+' } else { '#' });
        }
        out
    }

    pub fn san(&self, m: &Move) -> String {
        self.san_with(m, &self.legal_moves())
    }

    /// Find the legal move written in SAN. Check marks and annotations are optional.
    pub fn parse_san(&self, text: &str) -> Option<Move> {
        let clean = strip_san(text).replace("0-0-0", "O-O-O").replace("0-0", "O-O");
        let legal = self.legal_moves();
        legal
            .iter()
            .find(|m| strip_san(&self.san_with(m, &legal)) == clean)
            .copied()
    }

    /// Find a legal move by origin, destination and (for promotions) piece.
    pub fn find_move(&self, from: Square, to: Square, promotion: Option<PieceKind>) -> Option<Move> {
        self.legal_moves()
            .into_iter()
            .find(|m| m.from == from && m.to == to && m.promotion == promotion)
    }

    /// Text diagram of the board, White at the bottom.
    pub fn ascii(&self) -> String {
        let mut s = String::from("    a b c d e f g h\n   +----------------+\n");
        for rank in (0..8).rev() {
            s.push_str(&format!(" {} |", rank + 1));
            for file in 0..8 {
                let sq = (rank * 8 + file) as usize;
                let c = match self.squares[sq] {
                    Some(p) => p.fen_char(),
                    None => {
                        if (file + rank) % 2 == 0 {
                            ':'
                        } else {
                            '.'
                        }
                    }
                };
                s.push(c);
                s.push(' ');
            }
            s.pop();
            s.push_str(&format!("| {}\n", rank + 1));
        }
        s.push_str("   +----------------+\n    a b c d e f g h\n");
        s
    }
}

fn strip_san(text: &str) -> String {
    text.trim().trim_end_matches(['+', '#', '!', '?']).to_string()
}

impl fmt::Display for Board {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.ascii())
    }
}

/// Count legal move sequences of exactly `depth` plies (standard "perft").
pub fn perft(board: &Board, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    if depth == 1 {
        return board.count_legal_moves();
    }
    let mut moves = Vec::with_capacity(64);
    board.legal_moves_into(&mut moves);
    moves.iter().map(|m| perft(&board.play(m), depth - 1)).sum()
}
