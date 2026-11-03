//! The 20 chess strategies (Step 14.4; port of `strategies.ts`).
//!
//! Each strategy reads one position for one side and reports whether its
//! foundation, progress and completion conditions hold, which friendly pieces
//! participate, and which opposing pieces are its targets.

use crate::board::{file_of, parse_square, rank_of, square, square_name, Color, PieceKind, Square};
use crate::geometry::*;
use crate::position::{moves_for, EvalContext};

#[derive(Clone, Debug)]
pub struct StrategyReading {
    pub foundation: bool,
    pub progress: bool,
    pub completion: bool,
    pub participants: Vec<Unit>,
    pub targets: Vec<Unit>,
    pub evidence: String,
}

pub struct StrategyDefinition {
    pub id: &'static str,
    pub name: &'static str,
    pub intent: &'static str,
    pub foundation: &'static str,
    pub progress: &'static str,
    pub completion: &'static str,
    pub capture_link: &'static str,
    pub read: fn(&EvalContext) -> StrategyReading,
}

fn sq(name: &str) -> Square {
    parse_square(name).expect("valid square literal")
}

fn sqs(names: &[&str]) -> Vec<Square> {
    names.iter().map(|n| sq(n)).collect()
}

fn names(squares: &[Square], sep: &str) -> String {
    squares.iter().map(|s| square_name(*s)).collect::<Vec<_>>().join(sep)
}

fn empty(evidence: &str) -> StrategyReading {
    StrategyReading {
        foundation: false,
        progress: false,
        completion: false,
        participants: vec![],
        targets: vec![],
        evidence: evidence.to_string(),
    }
}

/// The three flags alone, for the six-move search (`EvalContext::flags_only`).
fn flags(foundation: bool, progress: bool, completion: bool) -> StrategyReading {
    StrategyReading { foundation, progress, completion, participants: vec![], targets: vec![], evidence: String::new() }
}

fn reading(
    foundation: bool,
    progress: bool,
    completion: bool,
    participants: Vec<Unit>,
    targets: Vec<Unit>,
    evidence: String,
) -> StrategyReading {
    StrategyReading {
        foundation,
        progress,
        completion,
        participants: uniq_units(participants),
        targets: uniq_units(targets.into_iter().filter(|u| u.kind != PieceKind::King).collect()),
        evidence,
    }
}

fn is_major(u: &Unit) -> bool {
    u.kind.is_major()
}

fn center() -> Vec<Square> {
    sqs(&["d4", "d5", "e4", "e5"])
}

fn side_central_squares(side: Color) -> Vec<Square> {
    match side {
        Color::White => sqs(&["d4", "e4"]),
        Color::Black => sqs(&["d5", "e5"]),
    }
}

/// Square one step ahead of a pawn of the given colour.
fn advance_square(u: &Unit) -> Option<Square> {
    square(file_of(u.square), rank_of(u.square) + u.color.forward())
}

// ---------------------------------------------------------------------------
// 1. Central pawn occupation
// ---------------------------------------------------------------------------

fn central_pawn_occupation(ctx: &EvalContext) -> StrategyReading {
    let pos = &ctx.pos;
    let side = ctx.side;
    let enemy = side.other();
    let central_pawns: Vec<Unit> = pos
        .of_kind(side, PieceKind::Pawn)
        .into_iter()
        .filter(|p| file_of(p.square) == 3 || file_of(p.square) == 4)
        .collect();
    let foundation = !central_pawns.is_empty();
    let occupied: Vec<Square> = side_central_squares(side)
        .into_iter()
        .filter(|&s| matches!(pos.at(s), Some(u) if u.color == side && u.kind == PieceKind::Pawn))
        .collect();
    let progress = !occupied.is_empty();
    let completion = occupied.len() == 2 && occupied.iter().all(|&s| !units_attacking(pos, s, side).is_empty());
    if !foundation && !progress && !completion {
        return empty("No friendly pawn remains on the d-file or the e-file.");
    }
    if ctx.flags_only {
        return flags(foundation, progress, completion);
    }
    let defenders: Vec<Unit> = occupied.iter().flat_map(|&s| units_attacking(pos, s, side)).collect();
    let zone_targets: Vec<Unit> = sqs(&["c4", "d4", "e4", "f4", "c5", "d5", "e5", "f5"])
        .into_iter()
        .filter_map(|s| pos.at(s))
        .filter(|u| u.color == enemy)
        .collect();
    let pawn_attackers: Vec<Unit> = occupied
        .iter()
        .flat_map(|&s| units_attacking(pos, s, enemy))
        .filter(|u| u.kind == PieceKind::Pawn)
        .collect();
    let occ_names = if occupied.is_empty() { "the d- or e-file".to_string() } else { names(&occupied, " and ") };
    let evidence = if completion {
        "Pawns occupy both central squares and each is defended.".to_string()
    } else if progress {
        format!("A pawn occupies {occ_names}.")
    } else {
        format!(
            "A pawn remains on the d-file or the e-file, not yet on {}.",
            names(&side_central_squares(side), " or ")
        )
    };
    let mut participants = central_pawns;
    participants.extend(defenders);
    let mut targets = zone_targets;
    targets.extend(pawn_attackers);
    reading(foundation, progress, completion, participants, targets, evidence)
}

// ---------------------------------------------------------------------------
// 2. Central pawn break
// ---------------------------------------------------------------------------

fn central_pawn_break(ctx: &EvalContext) -> StrategyReading {
    let pos = &ctx.pos;
    let side = ctx.side;
    let enemy = side.other();
    let enemy_central: Vec<Unit> = center()
        .into_iter()
        .filter_map(|s| pos.at(s))
        .filter(|u| u.color == enemy && u.kind == PieceKind::Pawn)
        .collect();
    let break_pawns: Vec<Unit> = pos
        .of_kind(side, PieceKind::Pawn)
        .into_iter()
        .filter(|p| (2..=5).contains(&file_of(p.square)))
        .collect();
    let foundation = !enemy_central.is_empty() && !break_pawns.is_empty();
    let hits = |pawn: &Unit, target: &Unit| attacked_squares(pos, pawn.square, PieceKind::Pawn, side).contains(&target.square);
    let attacking: Vec<Unit> = break_pawns
        .iter()
        .filter(|p| enemy_central.iter().any(|t| hits(p, t)))
        .copied()
        .collect();
    let progress = !attacking.is_empty();
    let completion = ctx.window.iter().any(|m| {
        m.color == side
            && m.piece == PieceKind::Pawn
            && m.captured == Some(PieceKind::Pawn)
            && (2..=5).contains(&file_of(m.to))
    });
    if !foundation && !progress && !completion {
        return empty("No central pawn break is prepared against an opposing central pawn.");
    }
    if ctx.flags_only {
        return flags(foundation, progress, completion);
    }
    let targets: Vec<Unit> = enemy_central
        .iter()
        .filter(|t| attacking.iter().any(|a| hits(a, t)))
        .copied()
        .collect();
    let evidence = if completion {
        "A pawn capture on the c-, d-, e-, or f-file is in the recorded turn window."
    } else if progress {
        "A pawn already attacks an opposing central pawn."
    } else {
        "Both sides have pawns that can meet in the center, and the break has not been played."
    };
    reading(foundation, progress, completion, break_pawns, targets, evidence.into())
}

// ---------------------------------------------------------------------------
// 3. Kingside pawn storm
// ---------------------------------------------------------------------------

fn kingside_pawn_storm(ctx: &EvalContext) -> StrategyReading {
    let pos = &ctx.pos;
    let side = ctx.side;
    let enemy = side.other();
    let king = pos.king(side);
    let storm: Vec<Unit> = pos.of_kind(side, PieceKind::Pawn).into_iter().filter(|p| file_of(p.square) >= 5).collect();
    let king_off = king.is_some_and(|k| file_of(k.square) <= 4);
    let moved = storm.iter().filter(|p| relative_rank(side, p.square) >= 2).count();
    let foundation = storm.len() >= 2 && moved >= 1 && king_off;
    let progress = moved >= 1 && king_off;
    let advanced = storm.iter().filter(|p| relative_rank(side, p.square) >= 3).count();
    let completion = advanced >= 2 && king.is_some_and(|k| file_of(k.square) <= 3);
    if !foundation && !progress && !completion {
        return empty("The f-, g-, and h-pawns have not started a storm away from the king.");
    }
    if ctx.flags_only {
        return flags(foundation, progress, completion);
    }
    let wing_targets: Vec<Unit> = pos
        .of(enemy)
        .into_iter()
        .filter(|u| file_of(u.square) >= 5 && u.kind != PieceKind::King)
        .collect();
    let helpers: Vec<Unit> = pos
        .of(side)
        .into_iter()
        .filter(|u| {
            u.kind != PieceKind::Pawn
                && u.kind != PieceKind::King
                && enemy_units_attacked_by(pos, u).iter().any(|t| file_of(t.square) >= 5)
        })
        .collect();
    let evidence = if completion {
        "Two kingside pawns have reached the fourth rank or beyond while the king is on the queenside."
    } else if progress {
        "A kingside pawn has left its starting square while the king is not on f, g, or h."
    } else {
        "Kingside pawns remain, but the storm has not advanced."
    };
    let mut participants = storm;
    participants.extend(helpers);
    reading(foundation, progress, completion, participants, wing_targets, evidence.into())
}

// ---------------------------------------------------------------------------
// 4. Queenside minority attack
// ---------------------------------------------------------------------------

fn is_doubled(pos: &Position, pawn: &Unit) -> bool {
    pos.of_kind(pawn.color, PieceKind::Pawn)
        .iter()
        .filter(|o| file_of(o.square) == file_of(pawn.square))
        .count()
        >= 2
}

fn queenside_minority(ctx: &EvalContext) -> StrategyReading {
    let pos = &ctx.pos;
    let side = ctx.side;
    let enemy = side.other();
    let own: Vec<Unit> = pos.of_kind(side, PieceKind::Pawn).into_iter().filter(|p| file_of(p.square) <= 2).collect();
    let enemy_wing = pos.of_kind(enemy, PieceKind::Pawn).into_iter().filter(|p| file_of(p.square) <= 2).count();
    let enemy_ext: Vec<Unit> = pos.of_kind(enemy, PieceKind::Pawn).into_iter().filter(|p| file_of(p.square) <= 3).collect();
    let foundation = enemy_wing > own.len() && !own.is_empty() && !enemy_ext.is_empty();
    let progress = own.iter().any(|p| relative_rank(side, p.square) >= 2);
    let deep = own.iter().any(|p| relative_rank(side, p.square) >= 3);
    let weakness = enemy_ext.iter().any(|p| is_isolated_pawn(pos, p) || is_doubled(pos, p));
    let completion = deep && weakness;
    if !foundation && !progress && !completion {
        return empty("There is no queenside pawn majority for the opponent to attack.");
    }
    if ctx.flags_only {
        return flags(foundation, progress, completion);
    }
    let majors: Vec<Unit> = pos.of(side).into_iter().filter(|u| is_major(u) && file_of(u.square) <= 3).collect();
    let evidence = if completion {
        "A queenside pawn has crossed into the opponent's half and an a–d pawn is isolated or doubled."
    } else if progress {
        "A queenside pawn has advanced against the larger pawn wing."
    } else {
        "The opponent has more pawns on a–c, and the minority pawns are still at home."
    };
    let mut participants = own;
    participants.extend(majors);
    reading(foundation, progress, completion, participants, enemy_ext, evidence.into())
}

// ---------------------------------------------------------------------------
// 5. King shelter
// ---------------------------------------------------------------------------

fn squares_between_rank(a: Square, b: Square) -> Vec<Square> {
    if rank_of(a) != rank_of(b) {
        return vec![];
    }
    let (lo, hi) = (file_of(a).min(file_of(b)), file_of(a).max(file_of(b)));
    ((lo + 1)..hi).filter_map(|f| square(f, rank_of(a))).collect()
}

fn king_shelter(ctx: &EvalContext) -> StrategyReading {
    let pos = &ctx.pos;
    let side = ctx.side;
    let Some(king) = pos.king(side) else {
        return empty("The king is missing.");
    };
    let rights = ctx.board.castling_rights(side);
    let back = if side == Color::White { "1" } else { "8" };
    let home = sq(&format!("e{back}"));
    let king_sq = sq(&format!("g{back}"));
    let queen_sq = sq(&format!("c{back}"));
    let on_castled = king.square == king_sq || king.square == queen_sq;
    let foundation = (king.square == home && (rights.king_side || rights.queen_side)) || on_castled;
    let path_cleared = |king_side: bool| {
        let has = if king_side { rights.king_side } else { rights.queen_side };
        if !has {
            return false;
        }
        let rook = sq(&format!("{}{back}", if king_side { "h" } else { "a" }));
        let between = squares_between_rank(king.square, rook);
        if between.is_empty() && king.square != home {
            return false;
        }
        between.iter().filter(|&&s| pos.at(s).is_some()).count() <= 1
    };
    let progress = on_castled || path_cleared(true) || path_cleared(false);
    let shield = |squares: &[Square]| {
        squares.iter().all(|&s| matches!(pos.at(s), Some(u) if u.color == side && u.kind == PieceKind::Pawn))
    };
    let king_safe = units_attacking(pos, king.square, side.other()).is_empty();
    let (ks_shield, qs_shield) = if side == Color::White {
        (sqs(&["f2", "g2", "h2"]), sqs(&["a2", "b2", "c2"]))
    } else {
        (sqs(&["f7", "g7", "h7"]), sqs(&["a7", "b7", "c7"]))
    };
    let kingside_ok = king.square == king_sq && king_safe && shield(&ks_shield);
    let queenside_ok = king.square == queen_sq && king_safe && shield(&qs_shield);
    let completion = kingside_ok || queenside_ok;
    if !foundation && !progress && !completion {
        return empty("Castling rights are gone and the king is not on a castled square.");
    }
    if ctx.flags_only {
        return flags(foundation, progress, completion);
    }
    let shelter: Vec<Square> = if kingside_ok {
        ks_shield
    } else if queenside_ok {
        qs_shield
    } else {
        vec![]
    };
    let shelter_pawns: Vec<Unit> = shelter.iter().filter_map(|&s| pos.at(s)).collect();
    let rooks: Vec<Unit> = pos
        .of_kind(side, PieceKind::Rook)
        .into_iter()
        .filter(|r| rank_of(r.square) == side.home_rank())
        .collect();
    let mut targets = units_attacking(pos, king.square, side.other());
    targets.extend(shelter.iter().flat_map(|&s| units_attacking(pos, s, side.other())));
    let evidence = if completion {
        "The king is on a castled square behind three unmoved shelter pawns and is not in check."
    } else if progress {
        if on_castled {
            "The king has reached a castled square, but the pawn shelter is incomplete or the king is in check."
        } else {
            "At most one piece remains between the king and a rook that can still castle."
        }
    } else {
        "The king is on its home square and at least one castling right remains."
    };
    let mut participants = vec![king];
    participants.extend(shelter_pawns);
    participants.extend(rooks);
    reading(foundation, progress, completion, participants, targets, evidence.into())
}

// ---------------------------------------------------------------------------
// 6. Opposite-side castling attack
// ---------------------------------------------------------------------------

fn opposite_castling_attack(ctx: &EvalContext) -> StrategyReading {
    let pos = &ctx.pos;
    let side = ctx.side;
    let enemy = side.other();
    let (Some(king), Some(enemy_king)) = (pos.king(side), pos.king(enemy)) else {
        return empty("A king is missing.");
    };
    let side_home = rank_of(king.square) == side.home_rank();
    let side_queenside = side_home && file_of(king.square) <= 2;
    let side_kingside = side_home && file_of(king.square) >= 5;
    let enemy_kingside = file_of(enemy_king.square) >= 5;
    let enemy_queenside = file_of(enemy_king.square) <= 2;
    let attack_kingside = side_queenside && enemy_kingside;
    let attack_queenside = side_kingside && enemy_queenside;
    let foundation = attack_kingside || attack_queenside;
    let wing: Vec<i32> = if attack_kingside {
        vec![5, 6, 7]
    } else if attack_queenside {
        vec![0, 1, 2]
    } else {
        vec![]
    };
    let wing_pawns: Vec<Unit> = pos
        .of_kind(side, PieceKind::Pawn)
        .into_iter()
        .filter(|p| wing.contains(&file_of(p.square)))
        .collect();
    let stepped = wing_pawns.iter().any(|p| relative_rank(side, p.square) >= 2);
    let deep = wing_pawns.iter().any(|p| relative_rank(side, p.square) >= 3);
    let progress = foundation && stepped;
    let major_on_half_open = pos.of(side).iter().any(|u| {
        is_major(u) && wing.contains(&file_of(u.square)) && pawns_on_file(pos, file_of(u.square), Some(side)).is_empty()
    });
    let completion = foundation && deep && major_on_half_open;
    if !foundation && !progress && !completion {
        return empty("The kings are not castled on opposite wings.");
    }
    if ctx.flags_only {
        return flags(foundation, progress, completion);
    }
    let majors: Vec<Unit> = pos.of(side).into_iter().filter(|u| is_major(u) && wing.contains(&file_of(u.square))).collect();
    let targets: Vec<Unit> = pos
        .of(enemy)
        .into_iter()
        .filter(|u| wing.contains(&file_of(u.square)) && u.kind != PieceKind::King)
        .collect();
    let evidence = if completion {
        "A pawn has reached the fourth rank on the enemy king's wing and a major piece stands on a half-open file there."
    } else if progress {
        "A pawn on the enemy king's wing has left its starting square."
    } else {
        "The kings are on opposite wings and the pawn storm has not started."
    };
    let mut participants = wing_pawns;
    participants.extend(majors);
    reading(foundation, progress, completion, participants, targets, evidence.into())
}

// ---------------------------------------------------------------------------
// 7. Fianchetto
// ---------------------------------------------------------------------------

fn fianchetto(ctx: &EvalContext) -> StrategyReading {
    let pos = &ctx.pos;
    let side = ctx.side;
    let wings: Vec<(Square, Square)> = match side {
        Color::White => vec![(sq("g3"), sq("g2")), (sq("b3"), sq("b2"))],
        Color::Black => vec![(sq("g6"), sq("g7")), (sq("b6"), sq("b7"))],
    };
    struct Wing {
        bishop_sq: Square,
        foundation: bool,
        progress: bool,
        completion: bool,
        pawn: Option<Unit>,
        bishop: Option<Unit>,
    }
    let read: Vec<Wing> = wings
        .iter()
        .map(|&(pawn_sq, bishop_sq)| {
            let pawn = pos.at(pawn_sq).filter(|u| u.color == side && u.kind == PieceKind::Pawn);
            let bishop = pos.at(bishop_sq).filter(|u| u.color == side && u.kind == PieceKind::Bishop);
            // Note: the TypeScript version gave b2/b7 the wrong square colour
            // (b2 is dark, b7 is light). This uses the square's real colour.
            let shade = square_shade(bishop_sq);
            let colour_alive = pos.of_kind(side, PieceKind::Bishop).iter().any(|b| square_shade(b.square) == shade);
            let bishop_home = bishop.is_some();
            let foundation = (pawn.is_some() || bishop_home) && (bishop_home || colour_alive);
            let completion = bishop_home
                && center().iter().any(|c| attacked_squares(pos, bishop_sq, PieceKind::Bishop, side).contains(c));
            Wing { bishop_sq, foundation, progress: bishop_home, completion, pawn, bishop }
        })
        .collect();
    let foundation = read.iter().any(|w| w.foundation);
    let progress = read.iter().any(|w| w.progress);
    let completion = read.iter().any(|w| w.completion);
    if !foundation && !progress && !completion {
        return empty("Neither the b-pawn nor the g-pawn has opened a fianchetto square.");
    }
    if ctx.flags_only {
        return flags(foundation, progress, completion);
    }
    let participants: Vec<Unit> = read
        .iter()
        .filter(|w| w.foundation || w.progress)
        .flat_map(|w| [w.pawn, w.bishop].into_iter().flatten())
        .collect();
    let targets: Vec<Unit> = read
        .iter()
        .filter_map(|w| w.bishop)
        .flat_map(|b| enemy_units_attacked_from(pos, b.square, PieceKind::Bishop, side))
        .collect();
    let placed: Vec<Square> = read.iter().filter(|w| w.progress).map(|w| w.bishop_sq).collect();
    let evidence = if completion {
        format!("A bishop on {} attacks a central square.", names(&placed, " or "))
    } else if progress {
        format!("A bishop occupies {}, but it does not attack the center.", names(&placed, " or "))
    } else {
        "The b- or g-pawn has stepped forward and the bishop of that color is still on the board.".into()
    };
    reading(foundation, progress, completion, participants, targets, evidence)
}

// ---------------------------------------------------------------------------
// 8. Open-file control
// ---------------------------------------------------------------------------

fn open_file_control(ctx: &EvalContext) -> StrategyReading {
    let pos = &ctx.pos;
    let side = ctx.side;
    let enemy = side.other();
    let half_open: Vec<i32> = (0..8).filter(|&f| pawns_on_file(pos, f, Some(side)).is_empty()).collect();
    let foundation = !half_open.is_empty() && pos.of(side).iter().any(is_major);
    let majors: Vec<Unit> = pos
        .of(side)
        .into_iter()
        .filter(|u| is_major(u) && half_open.contains(&file_of(u.square)))
        .collect();
    let progress = !majors.is_empty();
    let completion = (0..8).any(|f| {
        if !pawns_on_file(pos, f, None).is_empty() {
            return false;
        }
        let own = pos.of(side).iter().filter(|u| is_major(u) && file_of(u.square) == f).count();
        let theirs = pos.of(enemy).iter().filter(|u| is_major(u) && file_of(u.square) == f).count();
        own >= 1 && own > theirs
    });
    if !foundation && !progress && !completion {
        return empty("Every file that contains a friendly major piece also contains a friendly pawn.");
    }
    if ctx.flags_only {
        return flags(foundation, progress, completion);
    }
    let targets: Vec<Unit> = majors.iter().flat_map(|u| enemy_units_attacked_by(pos, u)).collect();
    let open_names: Vec<String> = (0..8)
        .filter(|&f| pawns_on_file(pos, f, None).is_empty())
        .map(|f| ((b'a' + f as u8) as char).to_string())
        .collect();
    let evidence = if completion {
        let list = if open_names.is_empty() { "none".to_string() } else { open_names.join(", ") };
        format!("A fully open file ({list}) holds more friendly major pieces than opposing ones.")
    } else if progress {
        "A rook or the queen has reached a file without a friendly pawn.".into()
    } else {
        "A file is free of friendly pawns, and a rook or queen is still on the board.".into()
    };
    let participants = if majors.is_empty() { pos.of(side).into_iter().filter(is_major).collect() } else { majors };
    reading(foundation, progress, completion, participants, targets, evidence)
}

// ---------------------------------------------------------------------------
// 9. Seventh-rank invasion
// ---------------------------------------------------------------------------

fn seventh_rank(ctx: &EvalContext) -> StrategyReading {
    let pos = &ctx.pos;
    let side = ctx.side;
    let invasion = if side == Color::White { 6 } else { 1 };
    let majors: Vec<Unit> = pos.of(side).into_iter().filter(is_major).collect();
    let entry = (0..8).any(|f| pawns_on_file(pos, f, Some(side)).is_empty());
    let foundation = !majors.is_empty() && entry;
    let advanced: Vec<Unit> = majors.iter().filter(|u| relative_rank(side, u.square) >= 5).copied().collect();
    let progress = !advanced.is_empty();
    let invaders: Vec<Unit> = majors.iter().filter(|u| rank_of(u.square) == invasion).copied().collect();
    let completion = invaders.iter().any(|u| !enemy_units_attacked_by(pos, u).is_empty());
    if !foundation && !progress && !completion {
        return empty("No rook or queen has a pawn-free file toward the seventh rank.");
    }
    if ctx.flags_only {
        return flags(foundation, progress, completion);
    }
    let targets: Vec<Unit> = advanced.iter().flat_map(|u| enemy_units_attacked_by(pos, u)).collect();
    let rank_label = if side == Color::White { "7" } else { "2" };
    let evidence = if completion {
        format!("A rook or queen on rank {rank_label} attacks an opposing piece.")
    } else if progress {
        "A major piece has reached the sixth rank or beyond, not yet the invasion rank with a target.".into()
    } else {
        "A major piece and a pawn-free file are available, but the piece is still in its own camp.".into()
    };
    reading(foundation, progress, completion, if progress { advanced } else { majors }, targets, evidence)
}

// ---------------------------------------------------------------------------
// 10. Passed-pawn advance
// ---------------------------------------------------------------------------

fn passed_pawn_advance(ctx: &EvalContext) -> StrategyReading {
    let pos = &ctx.pos;
    let side = ctx.side;
    let enemy = side.other();
    let passed: Vec<Unit> = pos.of_kind(side, PieceKind::Pawn).into_iter().filter(|p| is_passed_pawn(pos, p)).collect();
    let foundation = !passed.is_empty();
    let moved: Vec<Unit> = passed.iter().filter(|p| relative_rank(side, p.square) >= 3).copied().collect();
    let progress = !moved.is_empty();
    let deep: Vec<Unit> = passed
        .iter()
        .filter(|p| relative_rank(side, p.square) >= 5 && !units_attacking(pos, p.square, side).is_empty())
        .copied()
        .collect();
    let completion = !deep.is_empty();
    if !foundation && !progress && !completion {
        return empty("No friendly pawn is passed.");
    }
    if ctx.flags_only {
        return flags(foundation, progress, completion);
    }
    let featured = if completion { deep.clone() } else if progress { moved.clone() } else { passed.clone() };
    let defenders: Vec<Unit> = featured.iter().flat_map(|p| units_attacking(pos, p.square, side)).collect();
    let targets: Vec<Unit> = featured
        .iter()
        .flat_map(|p| {
            let mut out: Vec<Unit> = pos
                .of(enemy)
                .into_iter()
                .filter(|u| file_of(u.square) == file_of(p.square) && relative_rank(side, u.square) > relative_rank(side, p.square))
                .collect();
            out.extend(units_attacking(pos, p.square, enemy));
            if let Some(ahead) = advance_square(p).and_then(|s| pos.at(s)) {
                if ahead.color == enemy {
                    out.push(ahead);
                }
            }
            out
        })
        .collect();
    let evidence = if completion {
        format!("{} is passed, at least on the sixth rank, and defended.", join_squares(&deep, ", "))
    } else if progress {
        format!("A passed pawn has reached {}.", join_squares(&moved, ", "))
    } else {
        format!("A passed pawn exists on {} and has not reached the fourth rank.", join_squares(&passed, ", "))
    };
    let mut participants = featured;
    participants.extend(defenders);
    reading(foundation, progress, completion, participants, targets, evidence)
}

// ---------------------------------------------------------------------------
// 11. Isolated-pawn attack
// ---------------------------------------------------------------------------

fn isolated_pawn_attack(ctx: &EvalContext) -> StrategyReading {
    let pos = &ctx.pos;
    let side = ctx.side;
    let enemy = side.other();
    let on_file_major = |p: &Unit| pos.of(side).into_iter().filter(|u| is_major(u) && file_of(u.square) == file_of(p.square)).collect::<Vec<_>>();
    let isolated: Vec<Unit> = pos.of_kind(enemy, PieceKind::Pawn).into_iter().filter(|p| is_isolated_pawn(pos, p)).collect();
    let foundation = !isolated.is_empty();
    let pressured: Vec<Unit> = isolated
        .iter()
        .filter(|p| !units_attacking(pos, p.square, side).is_empty() || !on_file_major(p).is_empty())
        .copied()
        .collect();
    let progress = !pressured.is_empty();
    let winning: Vec<Unit> = isolated
        .iter()
        .filter(|p| units_attacking(pos, p.square, side).len() > units_attacking(pos, p.square, enemy).len())
        .copied()
        .collect();
    let completion = !winning.is_empty();
    if !foundation && !progress && !completion {
        return empty("The opponent has no isolated pawn.");
    }
    if ctx.flags_only {
        return flags(foundation, progress, completion);
    }
    let focus = if completion { &winning } else if progress { &pressured } else { &isolated };
    let participants: Vec<Unit> = focus
        .iter()
        .flat_map(|p| {
            let mut v = units_attacking(pos, p.square, side);
            v.extend(on_file_major(p));
            v
        })
        .collect();
    let targets: Vec<Unit> = focus
        .iter()
        .flat_map(|p| {
            let mut v = vec![*p];
            v.extend(units_attacking(pos, p.square, enemy).into_iter().filter(|u| u.kind != PieceKind::King));
            v
        })
        .collect();
    let evidence = if completion {
        format!("{} is isolated and attacked more times than it is defended.", join_squares(&winning, ", "))
    } else if progress {
        format!(
            "An isolated pawn on {} is under fire or shares a file with a major piece.",
            join_squares(&pressured, ", ")
        )
    } else {
        format!("An isolated pawn stands on {}.", join_squares(&isolated, ", "))
    };
    reading(foundation, progress, completion, participants, targets, evidence)
}

// ---------------------------------------------------------------------------
// 12. Backward-pawn pressure
// ---------------------------------------------------------------------------

fn is_backward_pawn(ctx: &EvalContext, pawn: &Unit) -> bool {
    let pos = &ctx.pos;
    let side = ctx.side;
    if pawn.kind != PieceKind::Pawn || pawn.color == side {
        return false;
    }
    let Some(advance) = advance_square(pawn) else {
        return false;
    };
    if !units_attacking(pos, advance, side).iter().any(|u| u.kind == PieceKind::Pawn) {
        return false;
    }
    // No pawn of the pawn's own side may defend the advance square.
    if units_attacking(pos, advance, pawn.color).iter().any(|u| u.kind == PieceKind::Pawn) {
        return false;
    }
    pawns_on_file(pos, file_of(pawn.square), Some(side)).is_empty()
}

fn backward_pawn_pressure(ctx: &EvalContext) -> StrategyReading {
    let pos = &ctx.pos;
    let side = ctx.side;
    let enemy = side.other();
    let file_majors = |p: &Unit| pos.of(side).into_iter().filter(|u| is_major(u) && file_of(u.square) == file_of(p.square)).collect::<Vec<_>>();
    let backward: Vec<Unit> = pos.of_kind(enemy, PieceKind::Pawn).into_iter().filter(|p| is_backward_pawn(ctx, p)).collect();
    let foundation = !backward.is_empty();
    let filed: Vec<Unit> = backward.iter().filter(|p| !file_majors(p).is_empty()).copied().collect();
    let progress = !filed.is_empty();
    let completed: Vec<Unit> = filed
        .iter()
        .filter(|p| match advance_square(p) {
            Some(adv) => pos.at(adv).is_some_and(|u| u.color == side) || !units_attacking(pos, adv, side).is_empty(),
            None => false,
        })
        .copied()
        .collect();
    let completion = !completed.is_empty();
    if !foundation && !progress && !completion {
        return empty("The opponent has no backward pawn on a half-open file.");
    }
    if ctx.flags_only {
        return flags(foundation, progress, completion);
    }
    let focus = if completion { &completed } else if progress { &filed } else { &backward };
    let participants: Vec<Unit> = focus
        .iter()
        .flat_map(|p| {
            let mut v = file_majors(p);
            if let Some(adv) = advance_square(p) {
                v.extend(units_attacking(pos, adv, side));
            }
            v
        })
        .collect();
    let targets: Vec<Unit> = focus
        .iter()
        .flat_map(|p| {
            let mut v = vec![*p];
            v.extend(units_attacking(pos, p.square, enemy));
            v
        })
        .collect();
    let evidence = if completion {
        format!(
            "A major piece bears on {} and the pawn's advance square is controlled.",
            join_squares(&completed, ", ")
        )
    } else if progress {
        format!("A rook or queen shares the file of the backward pawn on {}.", join_squares(&filed, ", "))
    } else {
        format!("A backward pawn stands on {}.", join_squares(&backward, ", "))
    };
    reading(foundation, progress, completion, participants, targets, evidence)
}

// ---------------------------------------------------------------------------
// 13. Knight outpost
// ---------------------------------------------------------------------------

fn enemy_pawn_can_still_attack(ctx: &EvalContext, target: Square) -> bool {
    let enemy = ctx.side.other();
    let target_rel = relative_rank(enemy, target);
    ctx.pos.of_kind(enemy, PieceKind::Pawn).iter().any(|p| {
        (file_of(p.square) - file_of(target)).abs() == 1 && relative_rank(enemy, p.square) < target_rel
    })
}

fn outpost_squares(ctx: &EvalContext) -> Vec<Square> {
    let pos = &ctx.pos;
    let side = ctx.side;
    let mut out = Vec::new();
    for file in 0..8 {
        for rank in 0..8 {
            let s = square(file, rank).unwrap();
            let rel = relative_rank(side, s);
            if !(3..=6).contains(&rel) {
                continue;
            }
            if let Some(occ) = pos.at(s) {
                if !(occ.color == side && occ.kind == PieceKind::Knight) {
                    continue;
                }
            }
            if !units_attacking(pos, s, side).iter().any(|u| u.kind == PieceKind::Pawn) {
                continue;
            }
            if enemy_pawn_can_still_attack(ctx, s) {
                continue;
            }
            out.push(s);
        }
    }
    out
}

fn knight_outpost(ctx: &EvalContext) -> StrategyReading {
    let pos = &ctx.pos;
    let side = ctx.side;
    let squares = outpost_squares(ctx);
    let foundation = !squares.is_empty();
    let knights = pos.of_kind(side, PieceKind::Knight);
    let occupying: Vec<Unit> = knights.iter().filter(|k| squares.contains(&k.square)).copied().collect();
    let eyeing: Vec<Unit> = knights
        .iter()
        .filter(|k| {
            let att = attacked_squares(pos, k.square, PieceKind::Knight, side);
            squares.iter().any(|s| att.contains(s))
        })
        .copied()
        .collect();
    let progress = !occupying.is_empty() || !eyeing.is_empty();
    let completion = !occupying.is_empty();
    if !foundation && !progress && !completion {
        return empty("No pawn-defended square in the opponent's half is safe from enemy pawns.");
    }
    if ctx.flags_only {
        return flags(foundation, progress, completion);
    }
    let relevant: Vec<Unit> = if completion {
        occupying.clone()
    } else if progress {
        occupying.iter().chain(eyeing.iter()).copied().collect()
    } else {
        vec![]
    };
    let defend_squares: Vec<Square> = if completion { occupying.iter().map(|k| k.square).collect() } else { squares.clone() };
    let defenders: Vec<Unit> = defend_squares
        .iter()
        .flat_map(|&s| units_attacking(pos, s, side).into_iter().filter(|u| u.kind == PieceKind::Pawn))
        .collect();
    let mut targets: Vec<Unit> = occupying.iter().flat_map(|k| enemy_units_attacked_by(pos, k)).collect();
    targets.extend(squares.iter().filter_map(|&s| pos.at(s)).filter(|u| u.color != side));
    let evidence = if completion {
        format!("A knight occupies the outpost on {}.", join_squares(&occupying, ", "))
    } else if progress {
        format!("A knight attacks an outpost square ({}).", names(&squares, ", "))
    } else {
        format!("An outpost square exists on {}, with no knight attending it.", names(&squares, ", "))
    };
    let mut participants = relevant;
    participants.extend(defenders);
    reading(foundation, progress, completion, participants, targets, evidence)
}

// ---------------------------------------------------------------------------
// 14. Bishop pair
// ---------------------------------------------------------------------------

fn bishop_pair(ctx: &EvalContext) -> StrategyReading {
    let pos = &ctx.pos;
    let side = ctx.side;
    let enemy = side.other();
    let bishops = pos.of_kind(side, PieceKind::Bishop);
    let enemy_bishops = pos.of_kind(enemy, PieceKind::Bishop).len();
    let foundation = bishops.len() == 2 && enemy_bishops < 2;
    let developed = bishops.iter().filter(|b| rank_of(b.square) != side.home_rank()).count();
    let progress = foundation && developed >= 1;
    let open_center = (2..=5).any(|f| pawns_on_file(pos, f, None).is_empty());
    let long_range = bishops.iter().any(|b| clear_travel(pos, b) >= 3);
    let completion = progress && open_center && long_range;
    if !foundation && !progress && !completion {
        return empty("The bishop pair is not ahead of the opponent's bishops.");
    }
    if ctx.flags_only {
        return flags(foundation, progress, completion);
    }
    let mut targets: Vec<Unit> = bishops.iter().flat_map(|b| enemy_units_attacked_by(pos, b)).collect();
    targets.extend(pos.of_kind(enemy, PieceKind::Knight).into_iter().filter(|n| {
        bishops
            .iter()
            .any(|b| attacked_squares(pos, b.square, PieceKind::Bishop, side).contains(&n.square))
    }));
    let evidence = if completion {
        "Both bishops remain, a central file is open, and a bishop has three clear squares of travel."
    } else if progress {
        "One bishop has left the back rank. The position is not yet open enough for the pair."
    } else {
        "Both bishops remain while the opponent has fewer, and both are still on the back rank."
    };
    reading(foundation, progress, completion, bishops, targets, evidence.into())
}

// ---------------------------------------------------------------------------
// 15. Pin  /  18. Skewer (shared line-up search)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Lineup {
    slider: Unit,
    front: Unit,
    rear: Unit,
    clear: bool,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum LineupKind {
    Pin,
    Skewer,
}

fn opposing_lineups(ctx: &EvalContext, kind: LineupKind) -> Vec<Lineup> {
    let pos = &ctx.pos;
    let side = ctx.side;
    let enemy = side.other();
    let mut out = Vec::new();
    for slider in pos.of(side).into_iter().filter(|u| u.kind.is_slider()) {
        for &(df, dr) in slider_dirs(slider.kind) {
            let ray = ray_units(pos, slider.square, df, dr);
            for front_i in 0..ray.len() {
                let rear_i = front_i + 1;
                if rear_i >= ray.len() || front_i > 1 {
                    continue;
                }
                let (front, rear) = (ray[front_i], ray[rear_i]);
                if front.color != enemy || rear.color != enemy {
                    continue;
                }
                if front_i == 1 && ray[0].color == side {
                    continue;
                }
                let (fv, rv) = (compare_value(front.kind), compare_value(rear.kind));
                if kind == LineupKind::Pin && !(rear.kind == PieceKind::King || rv > fv) {
                    continue;
                }
                if kind == LineupKind::Skewer && fv < rv {
                    continue;
                }
                out.push(Lineup { slider, front, rear, clear: front_i == 0 });
            }
        }
    }
    out
}

fn pin(ctx: &EvalContext) -> StrategyReading {
    let pos = &ctx.pos;
    let side = ctx.side;
    let lineups = opposing_lineups(ctx, LineupKind::Pin);
    let foundation = !lineups.is_empty();
    let clear: Vec<Lineup> = lineups.iter().filter(|l| l.clear).copied().collect();
    let progress = !clear.is_empty();
    let second = |l: &Lineup| -> Vec<Unit> {
        units_attacking(pos, l.front.square, side).into_iter().filter(|u| u.square != l.slider.square).collect()
    };
    let completion = clear.iter().any(|l| l.rear.kind == PieceKind::King || !second(l).is_empty());
    if !foundation && !progress && !completion {
        return empty("No slider is aligned with a lesser piece and a heavier piece behind it.");
    }
    if ctx.flags_only {
        return flags(foundation, progress, completion);
    }
    let relevant = if progress { &clear } else { &lineups };
    let mut participants: Vec<Unit> = relevant.iter().map(|l| l.slider).collect();
    participants.extend(clear.iter().flat_map(second));
    let targets: Vec<Unit> = relevant.iter().map(|l| l.front).collect();
    let evidence = if completion {
        format!(
            "The pin on {} is absolute or backed by a second attacker.",
            clear.iter().map(|l| square_name(l.front.square)).collect::<Vec<_>>().join(", ")
        )
    } else if progress {
        "A slider already attacks the front piece of a pin alignment.".into()
    } else {
        "A pin alignment exists, with one piece still blocking the slider.".into()
    };
    reading(foundation, progress, completion, participants, targets, evidence)
}

fn skewer(ctx: &EvalContext) -> StrategyReading {
    let pos = &ctx.pos;
    let side = ctx.side;
    let lineups = opposing_lineups(ctx, LineupKind::Skewer);
    let foundation = !lineups.is_empty();
    let clear: Vec<Lineup> = lineups
        .iter()
        .filter(|l| l.clear && (l.front.kind == PieceKind::King || compare_value(l.front.kind) > compare_value(l.rear.kind)))
        .copied()
        .collect();
    let progress = !clear.is_empty();
    let completion = clear.iter().any(|l| {
        if l.front.kind == PieceKind::King {
            return true;
        }
        let slider_value = material_value(l.slider.kind);
        let front_higher = compare_value(l.front.kind) > slider_value;
        let rear_loose = units_attacking(pos, l.rear.square, side.other()).is_empty();
        let rear_worth = compare_value(l.rear.kind) >= slider_value;
        front_higher && (rear_loose || rear_worth)
    });
    if !foundation && !progress && !completion {
        return empty("No slider is aligned with a heavier piece in front of a lighter one.");
    }
    if ctx.flags_only {
        return flags(foundation, progress, completion);
    }
    let relevant = if progress { &clear } else { &lineups };
    let evidence = if completion {
        format!(
            "The skewer on {} is forcing.",
            clear
                .iter()
                .map(|l| format!("{} in front of {}", square_name(l.front.square), square_name(l.rear.square)))
                .collect::<Vec<_>>()
                .join("; ")
        )
    } else if progress {
        "A slider attacks the front piece of a skewer, but the gain is not yet forcing.".into()
    } else {
        "A skewer alignment exists with one piece still in the way.".into()
    };
    reading(
        foundation,
        progress,
        completion,
        relevant.iter().map(|l| l.slider).collect(),
        relevant.iter().flat_map(|l| [l.front, l.rear]).collect(),
        evidence,
    )
}

// ---------------------------------------------------------------------------
// 16. Discovered attack
// ---------------------------------------------------------------------------

fn move_leaves_line(slider: Square, screen: Square, destination: Square) -> bool {
    let a = piece_count_between(None, slider, screen);
    let b = piece_count_between(None, slider, destination);
    if a.is_none() || b.is_none() {
        return true;
    }
    let same = (file_of(screen) - file_of(slider)).signum() == (file_of(destination) - file_of(slider)).signum()
        && (rank_of(screen) - rank_of(slider)).signum() == (rank_of(destination) - rank_of(slider)).signum();
    !same
}

fn discovered_attack(ctx: &EvalContext) -> StrategyReading {
    let pos = &ctx.pos;
    let side = ctx.side;
    let enemy = side.other();
    struct Discovery {
        slider: Unit,
        screen: Unit,
        target: Unit,
    }
    let mut discoveries = Vec::new();
    for slider in pos.of(side).into_iter().filter(|u| u.kind.is_slider()) {
        for &(df, dr) in slider_dirs(slider.kind) {
            let ray = ray_units(pos, slider.square, df, dr);
            if ray.len() < 2 {
                continue;
            }
            let (screen, target) = (ray[0], ray[1]);
            if screen.color != side || target.color != enemy || target.kind == PieceKind::Pawn {
                continue;
            }
            discoveries.push(Discovery { slider, screen, target });
        }
    }
    if discoveries.is_empty() {
        return empty("No friendly piece stands between a slider and an opposing piece other than a pawn.");
    }
    let legal = moves_for(ctx.board, side);
    let base = if ctx.board.turn() == side { Some(*ctx.board) } else { ctx.board.with_turn_passed() };
    let mut progress = false;
    let mut completion = false;
    let mut ready: Vec<Square> = Vec::new();
    let mut forcing: Vec<Square> = Vec::new();
    for d in &discoveries {
        for m in legal.iter().filter(|m| m.from == d.screen.square) {
            if !move_leaves_line(d.slider.square, d.screen.square, m.to) {
                continue;
            }
            progress = true;
            ready.push(d.screen.square);
            let Some(base) = base else { continue };
            let next = base.play(m);
            let Some(piece) = next.piece_at(d.slider.square) else { continue };
            let after = Position::read(&next);
            let attacks = attacked_squares(&after, d.slider.square, piece.kind, side).contains(&d.target.square);
            if attacks && (m.is_capture() || next.is_check()) {
                completion = true;
                forcing.push(d.screen.square);
                if ctx.flags_only {
                    return flags(true, true, true);
                }
            }
        }
    }
    if ctx.flags_only {
        return flags(true, progress, completion);
    }
    let participants: Vec<Unit> = discoveries
        .iter()
        .filter(|d| {
            if completion {
                forcing.contains(&d.screen.square)
            } else if progress {
                ready.contains(&d.screen.square)
            } else {
                true
            }
        })
        .flat_map(|d| [d.slider, d.screen])
        .collect();
    let targets: Vec<Unit> = discoveries
        .iter()
        .filter(|d| !completion || forcing.contains(&d.screen.square))
        .map(|d| d.target)
        .collect();
    let evidence = if completion {
        "Moving the screening piece captures or gives check and uncovers the slider."
    } else if progress {
        "The screening piece has a legal move off the line, but it does not yet capture or check."
    } else {
        "A friendly piece blocks a slider's line to an opposing piece and cannot move off that line."
    };
    reading(true, progress, completion, participants, targets, evidence.into())
}

// ---------------------------------------------------------------------------
// 17. Fork
// ---------------------------------------------------------------------------

fn fork_pair_qualifies(forker: PieceKind, targets: &[Unit], pos: &Position, color: Color, strict: bool) -> bool {
    if targets.len() < 2 {
        return false;
    }
    let mut ranked = targets.to_vec();
    ranked.sort_by_key(|b| std::cmp::Reverse(compare_value(b.kind)));
    if !strict && matches!(forker, PieceKind::Knight | PieceKind::Pawn) {
        return true;
    }
    let forker_value = compare_value(forker);
    ranked[..2].iter().all(|t| {
        t.kind == PieceKind::King
            || compare_value(t.kind) >= forker_value
            || units_attacking(pos, t.square, color.other()).is_empty()
    })
}

fn fork_targets_from(pos: &Position, from: Square, kind: PieceKind, color: Color, vacated: Option<Square>) -> Vec<Unit> {
    attacked_squares_ignoring(pos, from, kind, color, vacated)
        .into_iter()
        .filter_map(|s| pos.at(s))
        .filter(|u| u.color != color && u.kind != PieceKind::Pawn)
        .collect()
}

fn fork(ctx: &EvalContext) -> StrategyReading {
    let pos = &ctx.pos;
    let side = ctx.side;
    let (mut foundation, mut progress, mut completion) = (false, false, false);
    let mut participants = Vec::new();
    let mut targets = Vec::new();
    for mover in pos.of(side).into_iter().filter(|u| u.kind != PieceKind::King) {
        let current = fork_targets_from(pos, mover.square, mover.kind, side, None);
        if fork_pair_qualifies(mover.kind, &current, pos, side, false) {
            foundation = true;
            progress = true;
            participants.push(mover);
            if fork_pair_qualifies(mover.kind, &current, pos, side, true) {
                completion = true;
            }
            targets.extend(current);
        }
    }
    // With only the flags wanted, this pass can stop once the foundation is
    // known: it changes nothing else.
    if !completion && !(ctx.flags_only && foundation) {
        for m in moves_for(ctx.board, side) {
            if ctx.flags_only && foundation {
                break;
            }
            let kind = m.promotion.unwrap_or(m.piece);
            if kind == PieceKind::King {
                continue;
            }
            let landed: Vec<Unit> = fork_targets_from(pos, m.to, kind, side, Some(m.from)).into_iter().filter(|u| u.square != m.to).collect();
            let strict = !matches!(kind, PieceKind::Knight | PieceKind::Pawn);
            if !fork_pair_qualifies(kind, &landed, pos, side, strict) {
                continue;
            }
            foundation = true;
            if let Some(mover) = pos.at(m.from) {
                participants.push(mover);
            }
            targets.extend(landed);
        }
    }
    if !foundation && !progress && !completion {
        return empty("No piece attacks, or can move to a square attacking, two opposing pieces above pawn value.");
    }
    if ctx.flags_only {
        return flags(foundation, progress, completion);
    }
    let evidence = if completion {
        "A piece currently attacks two targets that are undefended, royal, or at least as valuable as the attacker."
    } else if progress {
        "A knight or pawn currently attacks two pieces, but the fork is not yet a material win."
    } else {
        "A legal move reaches a square that forks two qualifying pieces."
    };
    reading(foundation, progress, completion, participants, targets, evidence.into())
}

// ---------------------------------------------------------------------------
// 19. Major-piece battery
// ---------------------------------------------------------------------------

fn original_major_square(u: &Unit) -> bool {
    let n = square_name(u.square);
    match (u.kind, u.color) {
        (PieceKind::Queen, Color::White) => n == "d1",
        (PieceKind::Queen, Color::Black) => n == "d8",
        (PieceKind::Rook, Color::White) => n == "a1" || n == "h1",
        (PieceKind::Rook, Color::Black) => n == "a8" || n == "h8",
        _ => false,
    }
}

fn major_battery(ctx: &EvalContext) -> StrategyReading {
    let pos = &ctx.pos;
    let side = ctx.side;
    let majors: Vec<Unit> = pos.of(side).into_iter().filter(is_major).collect();
    struct Pair {
        a: Unit,
        b: Unit,
        between: usize,
    }
    let mut pairs = Vec::new();
    for i in 0..majors.len() {
        for j in (i + 1)..majors.len() {
            let (a, b) = (majors[i], majors[j]);
            if file_of(a.square) != file_of(b.square) && rank_of(a.square) != rank_of(b.square) {
                continue;
            }
            if let Some(between) = piece_count_between(Some(pos), a.square, b.square) {
                pairs.push(Pair { a, b, between });
            }
        }
    }
    let developed: Vec<&Pair> = pairs.iter().filter(|p| !original_major_square(&p.a) || !original_major_square(&p.b)).collect();
    let foundation = developed.iter().any(|p| p.between <= 2);
    let progress_pairs: Vec<&Pair> = developed.iter().filter(|p| p.between <= 1).copied().collect();
    let progress = !progress_pairs.is_empty();
    let completed: Vec<&Pair> = developed
        .iter()
        .filter(|p| {
            if p.between != 0 {
                return false;
            }
            let share_file = file_of(p.a.square) == file_of(p.b.square);
            let half_open = share_file && pawns_on_file(pos, file_of(p.a.square), Some(side)).is_empty();
            let bears = [p.a, p.b].iter().any(|piece| {
                enemy_units_attacked_by(pos, piece).iter().any(|t| {
                    if share_file {
                        file_of(t.square) == file_of(piece.square)
                    } else {
                        rank_of(t.square) == rank_of(piece.square)
                    }
                })
            });
            half_open || bears
        })
        .copied()
        .collect();
    let completion = !completed.is_empty();
    if !foundation && !progress && !completion {
        return empty("The rooks and queen are not gathered on a shared file or rank.");
    }
    if ctx.flags_only {
        return flags(foundation, progress, completion);
    }
    let chosen: Vec<&Pair> = if completion {
        completed
    } else if progress {
        progress_pairs
    } else {
        developed.iter().filter(|p| p.between <= 2).copied().collect()
    };
    let participants: Vec<Unit> = chosen.iter().flat_map(|p| [p.a, p.b]).collect();
    let targets: Vec<Unit> = participants.iter().flat_map(|u| enemy_units_attacked_by(pos, u)).collect();
    let evidence = if completion {
        "Two major pieces are connected and the line is half-open or bears on an opposing piece."
    } else if progress {
        "Two major pieces share a line with at most one piece between them."
    } else {
        "Two developed major pieces share a line with two pieces still between them."
    };
    reading(foundation, progress, completion, participants, targets, evidence.into())
}

// ---------------------------------------------------------------------------
// 20. Promotion
// ---------------------------------------------------------------------------

fn promotion(ctx: &EvalContext) -> StrategyReading {
    let pos = &ctx.pos;
    let side = ctx.side;
    let advanced: Vec<Unit> = pos.of_kind(side, PieceKind::Pawn).into_iter().filter(|p| relative_rank(side, p.square) >= 4).collect();
    let foundation = !advanced.is_empty();
    let deep: Vec<Unit> = advanced.iter().filter(|p| relative_rank(side, p.square) >= 5).copied().collect();
    let progress = !deep.is_empty();
    // A promotion can only be made by a pawn on the seventh rank, so without
    // one there is no promoting legal move to look for.
    let on_seventh = deep.iter().any(|p| relative_rank(side, p.square) == 6);
    let candidates = if on_seventh { moves_for(ctx.board, side) } else { Vec::new() };
    let promoting: Vec<Square> = candidates
        .into_iter()
        .filter(|m| m.piece == PieceKind::Pawn && m.promotion.is_some())
        .map(|m| m.from)
        .fold(Vec::new(), |mut acc, s| {
            if !acc.contains(&s) {
                acc.push(s);
            }
            acc
        });
    let completion = !promoting.is_empty();
    if !foundation && !progress && !completion {
        return empty("No pawn has reached the opponent's half of the board.");
    }
    if ctx.flags_only {
        return flags(foundation, progress, completion);
    }
    let featured: Vec<Unit> = if completion {
        advanced.iter().filter(|p| promoting.contains(&p.square)).copied().collect()
    } else if progress {
        deep
    } else {
        advanced
    };
    let defenders: Vec<Unit> = featured.iter().flat_map(|p| units_attacking(pos, p.square, side)).collect();
    let targets: Vec<Unit> = featured
        .iter()
        .flat_map(|p| {
            let mut v = units_attacking(pos, p.square, side.other());
            v.extend(
                attacked_squares(pos, p.square, PieceKind::Pawn, side)
                    .into_iter()
                    .filter_map(|s| pos.at(s))
                    .filter(|u| u.color != side),
            );
            if let Some(ahead) = advance_square(p).and_then(|s| pos.at(s)) {
                if ahead.color != side {
                    v.push(ahead);
                }
            }
            v
        })
        .collect();
    let evidence = if completion {
        format!("A pawn on {} has a legal promotion move.", names(&promoting, ", "))
    } else if progress {
        "A pawn has reached the sixth rank or beyond.".into()
    } else {
        "A pawn has entered the opponent's half.".into()
    };
    let mut participants = featured;
    participants.extend(defenders);
    reading(foundation, progress, completion, participants, targets, evidence)
}

// ---------------------------------------------------------------------------
// Definitions
// ---------------------------------------------------------------------------

pub static STRATEGIES: [StrategyDefinition; 20] = [
    StrategyDefinition {
        id: "central-pawn-occupation",
        name: "Central pawn occupation",
        intent: "Occupy d4 and e4, or d5 and e5, with pawns that the rest of the army defends.",
        foundation: "At least one friendly pawn stands on the d-file or the e-file.",
        progress: "A friendly pawn occupies the side's central squares: d4 or e4 for White, d5 or e5 for Black.",
        completion: "Friendly pawns occupy both of those squares, and each is geometrically defended.",
        capture_link: "The capturer is a central pawn or a piece defending one. The victim stands on c4–f5 or is a pawn attacking a friendly central pawn.",
        read: central_pawn_occupation,
    },
    StrategyDefinition {
        id: "central-pawn-break",
        name: "Central pawn break",
        intent: "Challenge an opposing central pawn with a c-, d-, e-, or f-pawn capture.",
        foundation: "The opponent has a pawn on d4, d5, e4, or e5, and a friendly pawn remains on the c-, d-, e-, or f-file.",
        progress: "A friendly pawn geometrically attacks an opposing pawn on one of those four squares.",
        completion: "The recorded window of the previous four turns, plus the current turn when it has been played, contains a friendly pawn capturing a pawn on the c-, d-, e-, or f-file.",
        capture_link: "The capturer is a c–f pawn. The victim is an opposing central pawn that pawn attacks.",
        read: central_pawn_break,
    },
    StrategyDefinition {
        id: "kingside-pawn-storm",
        name: "Kingside pawn storm",
        intent: "Advance the f-, g-, and h-pawns against the enemy king while the friendly king stays off that wing.",
        foundation: "At least two pawns remain on f–h, at least one has left its starting square, and the king is not on f, g, or h.",
        progress: "A pawn on f, g, or h has relative rank 2 or more, and the king is still off f–h.",
        completion: "At least two of those pawns have relative rank 3 or more, and the king is on files a–d.",
        capture_link: "The capturer is a storm pawn or a piece attacking the f–h wing. The victim stands on f, g, or h.",
        read: kingside_pawn_storm,
    },
    StrategyDefinition {
        id: "queenside-minority-attack",
        name: "Queenside minority attack",
        intent: "Advance a smaller queenside pawn mass to leave the opponent with an isolated or doubled pawn.",
        foundation: "On files a–c the opponent has strictly more pawns, the side has at least one, and the opponent has a pawn on a–d.",
        progress: "A friendly pawn on a, b, or c has relative rank 2 or more.",
        completion: "A friendly pawn on a–c has relative rank 3 or more, and an opposing pawn on a–d is isolated or doubled.",
        capture_link: "The capturer is an a–c pawn or a major piece on a–d. The victim is an opposing pawn on a–d.",
        read: queenside_minority,
    },
    StrategyDefinition {
        id: "king-shelter",
        name: "King shelter",
        intent: "Castle and keep the three pawns in front of the king on their original squares.",
        foundation: "The king is on e1 or e8 with at least one castling right, or the king is already on c1, g1, c8, or g8.",
        progress: "The king is on a castled square, or at most one piece remains between the king and a rook that still has castling rights.",
        completion: "The king is on g1, g8, c1, or c8, is not attacked, and the three shelter pawns on that wing are unmoved.",
        capture_link: "The capturer is the king, a shelter pawn, or a back-rank rook. The victim attacks the king or a shelter pawn.",
        read: king_shelter,
    },
    StrategyDefinition {
        id: "opposite-side-castling-attack",
        name: "Opposite-side castling attack",
        intent: "Storm the wing where the enemy king lives after the kings have castled on opposite sides.",
        foundation: "The friendly king is on the home rank on files a–c or f–h, and the enemy king is on the other wing (files f–h or a–c).",
        progress: "A friendly pawn on the enemy king's wing has relative rank 2 or more.",
        completion: "One of those pawns has relative rank 3 or more, and a friendly rook or queen stands on a half-open file of that wing.",
        capture_link: "The capturer is a wing pawn or a major piece on that wing. The victim stands on the enemy king's wing.",
        read: opposite_castling_attack,
    },
    StrategyDefinition {
        id: "fianchetto",
        name: "Fianchetto",
        intent: "Place a bishop on the long diagonal behind a one-step b- or g-pawn and aim it at the center.",
        foundation: "The b- or g-pawn has moved exactly one square, or the bishop is already on b2, g2, b7, or g7, and that color of bishop still exists.",
        progress: "The bishop occupies the fianchetto square.",
        completion: "That bishop geometrically attacks d4, d5, e4, or e5.",
        capture_link: "The capturer is the fianchetto bishop or the pawn that opened its diagonal. The victim is a piece the bishop attacks.",
        read: fianchetto,
    },
    StrategyDefinition {
        id: "open-file-control",
        name: "Open-file control",
        intent: "Put a rook or queen on a file with no friendly pawn, and own a fully open file.",
        foundation: "A file has no friendly pawn, and a friendly rook or queen is still on the board.",
        progress: "A friendly rook or queen stands on a file with no friendly pawn.",
        completion: "A fully open file contains more friendly rooks and queens than opposing ones.",
        capture_link: "The capturer is a rook or queen on a half-open file. The victim is a piece that major piece attacks.",
        read: open_file_control,
    },
    StrategyDefinition {
        id: "seventh-rank-invasion",
        name: "Seventh-rank invasion",
        intent: "Bring a rook or queen to the opponent's second rank and attack something there.",
        foundation: "A friendly rook or queen exists, and at least one file has no friendly pawn.",
        progress: "A friendly rook or queen has relative rank 5 or more.",
        completion: "A friendly rook or queen is on the opponent's second rank and geometrically attacks an opposing unit.",
        capture_link: "The capturer is a major piece on the sixth rank or beyond. The victim is a piece that invader attacks.",
        read: seventh_rank,
    },
    StrategyDefinition {
        id: "passed-pawn-advance",
        name: "Passed-pawn advance",
        intent: "Create a passed pawn, escort it past the midline, and defend it on the sixth rank or beyond.",
        foundation: "A friendly pawn has no opposing pawn on its file or an adjacent file in front of it.",
        progress: "That passed pawn has relative rank 3 or more.",
        completion: "A passed pawn has relative rank 5 or more and is geometrically defended.",
        capture_link: "The capturer is the passed pawn or a piece defending it. The victim blocks it, attacks it, or stands on the square in front of it.",
        read: passed_pawn_advance,
    },
    StrategyDefinition {
        id: "isolated-pawn-attack",
        name: "Isolated-pawn attack",
        intent: "Fix an opposing pawn that has no neighbor, then attack it more times than it is defended.",
        foundation: "The opponent has a pawn with no pawn of its own colour on an adjacent file.",
        progress: "A friendly unit geometrically attacks that pawn, or a friendly rook or queen shares its file.",
        completion: "The isolated pawn is geometrically attacked more times than it is defended.",
        capture_link: "The capturer attacks the isolated pawn or shares its file as a rook or queen. The victim is the isolated pawn or a piece defending it.",
        read: isolated_pawn_attack,
    },
    StrategyDefinition {
        id: "backward-pawn-pressure",
        name: "Backward-pawn pressure",
        intent: "Restrain a pawn whose advance square is hit by a friendly pawn, then line a major piece up against it.",
        foundation: "An opposing pawn's advance square is attacked by a friendly pawn, no opposing pawn defends it, and its file has no friendly pawn.",
        progress: "A friendly rook or queen shares that pawn's file.",
        completion: "The advance square is occupied or attacked by the side, and the major piece remains on the file.",
        capture_link: "The capturer is the major piece or a unit controlling the advance square. The victim is the backward pawn or a piece defending it.",
        read: backward_pawn_pressure,
    },
    StrategyDefinition {
        id: "knight-outpost",
        name: "Knight outpost",
        intent: "Plant a knight on a square in the opponent's half that a friendly pawn defends and no enemy pawn can attack.",
        foundation: "Such a square exists on relative ranks 3 through 6. It is empty or already occupied by a friendly knight.",
        progress: "A friendly knight occupies the square or geometrically attacks it.",
        completion: "A friendly knight occupies the square.",
        capture_link: "The capturer is the knight or the pawn that defends the outpost. The victim is a piece the occupying knight attacks.",
        read: knight_outpost,
    },
    StrategyDefinition {
        id: "bishop-pair",
        name: "Bishop pair",
        intent: "Keep both bishops against fewer opposing bishops and open a central file for them.",
        foundation: "The side has two bishops and the opponent has fewer than two.",
        progress: "At least one friendly bishop has left the back rank.",
        completion: "A file from c through f is empty of pawns, and a friendly bishop has at least three squares of clear travel.",
        capture_link: "The capturer is one of the two bishops. The victim is a piece that bishop attacks.",
        read: bishop_pair,
    },
    StrategyDefinition {
        id: "pin",
        name: "Pin",
        intent: "Align a bishop, rook, or queen against a piece that cannot step aside without exposing a heavier piece or the king.",
        foundation: "A friendly slider, an opposing front piece, and a heavier opposing rear piece or the king share a line, with at most one non-friendly piece between the slider and the front piece and nothing between the two opposing pieces.",
        progress: "Nothing remains between the slider and the front piece.",
        completion: "The line is clear and either the rear piece is the king or a second friendly unit also attacks the front piece.",
        capture_link: "The capturer is the pinning slider or the second attacker. The victim is the pinned piece.",
        read: pin,
    },
    StrategyDefinition {
        id: "discovered-attack",
        name: "Discovered attack",
        intent: "Move a piece off a slider's line so the slider's attack arrives as the mover captures or gives check.",
        foundation: "Exactly one friendly piece stands between a friendly slider and an opposing piece other than a pawn.",
        progress: "That screening piece has a legal move off the line.",
        completion: "One of those moves captures or gives check, and the slider still attacks the original target afterward.",
        capture_link: "The capturer is the slider or the screening piece. The victim is the discovered target.",
        read: discovered_attack,
    },
    StrategyDefinition {
        id: "fork",
        name: "Fork",
        intent: "Attack two pieces of at least knight value with one lesser piece, or reach the square that does it.",
        foundation: "A legal move, or the piece's current square, attacks two opposing units other than pawns. Slider forks count only when both targets are royal, undefended, or at least as valuable as the slider.",
        progress: "A knight or pawn currently attacks two such pieces, or a slider currently has a qualifying pair.",
        completion: "Two current targets are each a king, geometrically undefended, or worth at least as much as the forker.",
        capture_link: "The capturer is the forking piece. The victim is one of the forked pieces.",
        read: fork,
    },
    StrategyDefinition {
        id: "skewer",
        name: "Skewer",
        intent: "Attack a more valuable piece that must step off a line and leave a lesser piece behind it.",
        foundation: "A friendly slider is aligned with two opposing pieces, the nearer one at least as valuable as the farther one, with at most one non-friendly blocker in front.",
        progress: "The line to the front piece is clear, and the front piece is the king or strictly more valuable than the piece behind it.",
        completion: "The front piece is the king, or it is worth more than the slider and the rear piece is undefended or worth at least the slider.",
        capture_link: "The capturer is the slider. The victim is the front piece or the piece behind it.",
        read: skewer,
    },
    StrategyDefinition {
        id: "major-piece-battery",
        name: "Major-piece battery",
        intent: "Double rooks, or a rook and queen, on a file or rank that is open or aimed at a target.",
        foundation: "Two friendly rooks or queens share a file or rank, at most two pieces stand between them, and at least one has left its original square.",
        progress: "At most one piece stands between them.",
        completion: "Nothing stands between them, and they share a half-open file or one of them attacks an opposing piece along the line.",
        capture_link: "The capturer is one of the battery pieces. The victim is a piece that battery piece attacks.",
        read: major_battery,
    },
    StrategyDefinition {
        id: "promotion",
        name: "Promotion",
        intent: "Escort a pawn into the opponent's half and give it a legal move onto the last rank.",
        foundation: "A friendly pawn has relative rank 4 or more.",
        progress: "A friendly pawn has relative rank 5 or more.",
        completion: "A friendly pawn has a legal move that promotes.",
        capture_link: "The capturer is the advanced pawn or a piece defending it. The victim attacks that pawn, stands in front of it, or can be taken by it.",
        read: promotion,
    },
];

impl std::fmt::Debug for StrategyDefinition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StrategyDefinition({})", self.id)
    }
}

pub fn strategy_by_id(id: &str) -> Option<&'static StrategyDefinition> {
    STRATEGIES.iter().find(|s| s.id == id)
}
