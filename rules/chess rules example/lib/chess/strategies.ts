import type { Square } from "chess.js";
import {
  attackedSquares,
  clearTravel,
  compareValue,
  enemyUnitsAttackedBy,
  enemyUnitsAttackedFrom,
  fileIndex,
  homeRank,
  isIsolatedPawn,
  isPassedPawn,
  materialValue,
  opposite,
  pawnsOnFile,
  pieceCountBetween,
  rankIndex,
  rayUnits,
  relativeRank,
  sliderDirs,
  squareAt,
  squareShade,
  type Color,
  type PieceSymbol,
  type Unit,
  uniqUnits,
  unitsAttacking,
} from "./geometry";
import { cloneChess, movesFor, readPosition, type EvalContext } from "./position";

export interface StrategyReading {
  foundation: boolean;
  progress: boolean;
  completion: boolean;
  participants: Unit[];
  targets: Unit[];
  evidence: string;
}

export interface StrategyDefinition {
  id: string;
  name: string;
  intent: string;
  foundation: string;
  progress: string;
  completion: string;
  captureLink: string;
  read(ctx: EvalContext): StrategyReading;
}

const CENTER: Square[] = ["d4", "d5", "e4", "e5"];
const CENTRAL_BELT: Square[] = ["c4", "d4", "e4", "f4", "c5", "d5", "e5", "f5"];

function empty(evidence: string): StrategyReading {
  return {
    foundation: false,
    progress: false,
    completion: false,
    participants: [],
    targets: [],
    evidence,
  };
}

function reading(
  flags: { foundation: boolean; progress: boolean; completion: boolean },
  participants: Unit[],
  targets: Unit[],
  evidence: string,
): StrategyReading {
  return {
    ...flags,
    participants: uniqUnits(participants),
    targets: uniqUnits(targets.filter((unit) => unit.type !== "k")),
    evidence,
  };
}

function sideCentralSquares(side: Color): Square[] {
  return side === "w" ? ["d4", "e4"] : ["d5", "e5"];
}

function centralPawnOccupation(ctx: EvalContext): StrategyReading {
  const { pos, side } = ctx;
  const enemy = opposite(side);
  const centralPawns = pos
    .of(side, "p")
    .filter((pawn) => fileIndex(pawn.square) === 3 || fileIndex(pawn.square) === 4);
  const foundation = centralPawns.length > 0;
  const occupied = sideCentralSquares(side).filter((square) => {
    const unit = pos.at(square);
    return unit?.color === side && unit.type === "p";
  });
  const progress = occupied.length > 0;
  const completion =
    occupied.length === 2 &&
    occupied.every((square) => unitsAttacking(pos, square, side).length > 0);
  if (!foundation && !progress && !completion) {
    return empty("No friendly pawn remains on the d-file or the e-file.");
  }
  const defenders = occupied.flatMap((square) => unitsAttacking(pos, square, side));
  const zoneTargets = CENTRAL_BELT.flatMap((square) => {
    const unit = pos.at(square);
    return unit && unit.color === enemy ? [unit] : [];
  });
  const pawnAttackers = occupied.flatMap((square) =>
    unitsAttacking(pos, square, enemy).filter((unit) => unit.type === "p"),
  );
  const names = occupied.length > 0 ? occupied.join(" and ") : "the d- or e-file";
  return reading(
    { foundation, progress, completion },
    [...centralPawns, ...defenders],
    [...zoneTargets, ...pawnAttackers],
    completion
      ? `Pawns occupy both central squares and each is defended.`
      : progress
        ? `A pawn occupies ${names}.`
        : `A pawn remains on the d-file or the e-file, not yet on ${sideCentralSquares(side).join(" or ")}.`,
  );
}

function centralPawnBreak(ctx: EvalContext): StrategyReading {
  const { pos, side, window } = ctx;
  const enemy = opposite(side);
  const enemyCentral = CENTER.flatMap((square) => {
    const unit = pos.at(square);
    return unit?.color === enemy && unit.type === "p" ? [unit] : [];
  });
  const breakPawns = pos
    .of(side, "p")
    .filter((pawn) => {
      const file = fileIndex(pawn.square);
      return file >= 2 && file <= 5;
    });
  const foundation = enemyCentral.length > 0 && breakPawns.length > 0;
  const attacking = breakPawns.filter((pawn) =>
    enemyCentral.some((target) =>
      attackedSquares(pos, pawn.square, "p", side).includes(target.square),
    ),
  );
  const progress = attacking.length > 0;
  const completion = window.some(
    (move) =>
      move.color === side &&
      move.piece === "p" &&
      move.captured === "p" &&
      "cdef".includes(move.to[0]),
  );
  if (!foundation && !progress && !completion) {
    return empty("No central pawn break is prepared against an opposing central pawn.");
  }
  const targets = enemyCentral.filter((pawn) =>
    attacking.some((attacker) =>
      attackedSquares(pos, attacker.square, "p", side).includes(pawn.square),
    ),
  );
  return reading(
    { foundation, progress, completion },
    breakPawns,
    targets,
    completion
      ? `A pawn capture on the c-, d-, e-, or f-file is in the recorded turn window.`
      : progress
        ? `A pawn already attacks an opposing central pawn.`
        : `Both sides have pawns that can meet in the center, and the break has not been played.`,
  );
}

function kingsidePawnStorm(ctx: EvalContext): StrategyReading {
  const { pos, side } = ctx;
  const enemy = opposite(side);
  const king = pos.king(side);
  const stormPawns = pos.of(side, "p").filter((pawn) => fileIndex(pawn.square) >= 5);
  const kingOffStorm = king ? fileIndex(king.square) <= 4 : false;
  const moved = stormPawns.filter((pawn) => relativeRank(side, pawn.square) >= 2);
  const foundation = stormPawns.length >= 2 && moved.length >= 1 && kingOffStorm;
  const progress = moved.length >= 1 && kingOffStorm;
  const advanced = stormPawns.filter((pawn) => relativeRank(side, pawn.square) >= 3);
  const completion = advanced.length >= 2 && !!king && fileIndex(king.square) <= 3;
  if (!foundation && !progress && !completion) {
    return empty("The f-, g-, and h-pawns have not started a storm away from the king.");
  }
  const wingTargets = pos
    .of(enemy)
    .filter((unit) => fileIndex(unit.square) >= 5 && unit.type !== "k");
  const helpers = pos.of(side).filter((unit) => {
    if (unit.type === "p" || unit.type === "k") return false;
    return enemyUnitsAttackedBy(pos, unit).some((target) => fileIndex(target.square) >= 5);
  });
  return reading(
    { foundation, progress, completion },
    [...stormPawns, ...helpers],
    wingTargets,
    completion
      ? `Two kingside pawns have reached the fourth rank or beyond while the king is on the queenside.`
      : progress
        ? `A kingside pawn has left its starting square while the king is not on f, g, or h.`
        : `Kingside pawns remain, but the storm has not advanced.`,
  );
}

function isDoubled(pos: EvalContext["pos"], pawn: Unit): boolean {
  return pos.of(pawn.color, "p").filter((other) => fileIndex(other.square) === fileIndex(pawn.square)).length >= 2;
}

function queensideMinority(ctx: EvalContext): StrategyReading {
  const { pos, side } = ctx;
  const enemy = opposite(side);
  const own = pos.of(side, "p").filter((pawn) => fileIndex(pawn.square) <= 2);
  const enemyWing = pos.of(enemy, "p").filter((pawn) => fileIndex(pawn.square) <= 2);
  const enemyExtended = pos.of(enemy, "p").filter((pawn) => fileIndex(pawn.square) <= 3);
  const foundation = enemyWing.length > own.length && own.length >= 1 && enemyExtended.length >= 1;
  const moved = own.filter((pawn) => relativeRank(side, pawn.square) >= 2);
  const progress = moved.length >= 1;
  const deep = own.filter((pawn) => relativeRank(side, pawn.square) >= 3);
  const weakness = enemyExtended.some((pawn) => isIsolatedPawn(pos, pawn) || isDoubled(pos, pawn));
  const completion = deep.length >= 1 && weakness;
  if (!foundation && !progress && !completion) {
    return empty("There is no queenside pawn majority for the opponent to attack.");
  }
  const majors = pos
    .of(side)
    .filter(
      (unit) =>
        (unit.type === "r" || unit.type === "q") && fileIndex(unit.square) <= 3,
    );
  return reading(
    { foundation, progress, completion },
    [...own, ...majors],
    enemyExtended,
    completion
      ? `A queenside pawn has crossed into the opponent's half and an a–d pawn is isolated or doubled.`
      : progress
        ? `A queenside pawn has advanced against the larger pawn wing.`
        : `The opponent has more pawns on a–c, and the minority pawns are still at home.`,
  );
}

function squaresBetweenRank(a: Square, b: Square): Square[] {
  if (rankIndex(a) !== rankIndex(b)) return [];
  const left = Math.min(fileIndex(a), fileIndex(b));
  const right = Math.max(fileIndex(a), fileIndex(b));
  const squares: Square[] = [];
  for (let file = left + 1; file < right; file += 1) {
    const square = squareAt(file, rankIndex(a));
    if (square) squares.push(square);
  }
  return squares;
}

function kingShelter(ctx: EvalContext): StrategyReading {
  const { pos, chess, side } = ctx;
  const king = pos.king(side);
  if (!king) return empty("The king is missing.");
  const rights = chess.getCastlingRights(side);
  const home = `${"e"}${side === "w" ? "1" : "8"}` as Square;
  const kingsideSquare = (side === "w" ? "g1" : "g8") as Square;
  const queensideSquare = (side === "w" ? "c1" : "c8") as Square;
  const onCastledSquare = king.square === kingsideSquare || king.square === queensideSquare;
  const foundation = (king.square === home && (rights.k || rights.q)) || onCastledSquare;
  const rookSquare = (file: "a" | "h"): Square =>
    `${file}${side === "w" ? "1" : "8"}` as Square;
  const pathCleared = (wing: "k" | "q") => {
    if (!rights[wing]) return false;
    const rook = rookSquare(wing === "k" ? "h" : "a");
    const between = squaresBetweenRank(king.square, rook);
    if (between.length === 0 && king.square !== home) return false;
    const blockers = between.filter((square) => pos.at(square)).length;
    return blockers <= 1;
  };
  const progress = onCastledSquare || pathCleared("k") || pathCleared("q");
  const shield = (squares: Square[]) => squares.every((square) => {
    const unit = pos.at(square);
    return unit?.color === side && unit.type === "p";
  });
  const kingSafe = unitsAttacking(pos, king.square, opposite(side)).length === 0;
  const kingsideShield =
    king.square === kingsideSquare &&
    kingSafe &&
    shield(side === "w" ? ["f2", "g2", "h2"] : ["f7", "g7", "h7"]);
  const queensideShield =
    king.square === queensideSquare &&
    kingSafe &&
    shield(side === "w" ? ["a2", "b2", "c2"] : ["a7", "b7", "c7"]);
  const completion = kingsideShield || queensideShield;
  if (!foundation && !progress && !completion) {
    return empty("Castling rights are gone and the king is not on a castled square.");
  }
  const shelterSquares: Square[] = kingsideShield
    ? side === "w"
      ? ["f2", "g2", "h2"]
      : ["f7", "g7", "h7"]
    : queensideShield
      ? side === "w"
        ? ["a2", "b2", "c2"]
        : ["a7", "b7", "c7"]
      : [];
  const shelterPawns = shelterSquares.flatMap((square) => {
    const unit = pos.at(square);
    return unit ? [unit] : [];
  });
  const rook = pos
    .of(side, "r")
    .filter((unit) => rankIndex(unit.square) === homeRank(side));
  const attackers = unitsAttacking(pos, king.square, opposite(side));
  const shieldAttackers = shelterSquares.flatMap((square) =>
    unitsAttacking(pos, square, opposite(side)),
  );
  return reading(
    { foundation, progress, completion },
    [king, ...shelterPawns, ...rook],
    [...attackers, ...shieldAttackers],
    completion
      ? `The king is on a castled square behind three unmoved shelter pawns and is not in check.`
      : progress
        ? onCastledSquare
          ? `The king has reached a castled square, but the pawn shelter is incomplete or the king is in check.`
          : `At most one piece remains between the king and a rook that can still castle.`
        : `The king is on its home square and at least one castling right remains.`,
  );
}

function oppositeCastlingAttack(ctx: EvalContext): StrategyReading {
  const { pos, side } = ctx;
  const enemy = opposite(side);
  const king = pos.king(side);
  const enemyKing = pos.king(enemy);
  if (!king || !enemyKing) return empty("A king is missing.");
  const sideHome = rankIndex(king.square) === homeRank(side);
  const sideQueenside = sideHome && fileIndex(king.square) <= 2;
  const sideKingside = sideHome && fileIndex(king.square) >= 6;
  const enemyKingside = fileIndex(enemyKing.square) >= 5;
  const enemyQueenside = fileIndex(enemyKing.square) <= 2;
  const attackKingside = sideQueenside && enemyKingside;
  const attackQueenside = sideKingside && enemyQueenside;
  const foundation = attackKingside || attackQueenside;
  const wingFiles = attackKingside ? [5, 6, 7] : attackQueenside ? [0, 1, 2] : [];
  const wingPawns = pos
    .of(side, "p")
    .filter((pawn) => wingFiles.includes(fileIndex(pawn.square)));
  const stepped = wingPawns.filter((pawn) => relativeRank(side, pawn.square) >= 2);
  const deep = wingPawns.filter((pawn) => relativeRank(side, pawn.square) >= 3);
  const progress = foundation && stepped.length >= 1;
  const majorOnHalfOpen = pos.of(side).some((unit) => {
    if (unit.type !== "r" && unit.type !== "q") return false;
    const file = fileIndex(unit.square);
    return wingFiles.includes(file) && pawnsOnFile(pos, file, side).length === 0;
  });
  const completion = foundation && deep.length >= 1 && majorOnHalfOpen;
  if (!foundation && !progress && !completion) {
    return empty("The kings are not castled on opposite wings.");
  }
  const majors = pos.of(side).filter((unit) => {
    if (unit.type !== "r" && unit.type !== "q") return false;
    return wingFiles.includes(fileIndex(unit.square));
  });
  const targets = pos
    .of(enemy)
    .filter((unit) => wingFiles.includes(fileIndex(unit.square)) && unit.type !== "k");
  return reading(
    { foundation, progress, completion },
    [...wingPawns, ...majors],
    targets,
    completion
      ? `A pawn has reached the fourth rank on the enemy king's wing and a major piece stands on a half-open file there.`
      : progress
        ? `A pawn on the enemy king's wing has left its starting square.`
        : `The kings are on opposite wings and the pawn storm has not started.`,
  );
}

interface FianchettoWing {
  pawn: Square;
  bishop: Square;
  shade: "light" | "dark";
}

function fianchettoWings(side: Color): FianchettoWing[] {
  if (side === "w") {
    return [
      { pawn: "g3", bishop: "g2", shade: "light" },
      { pawn: "b3", bishop: "b2", shade: "light" },
    ];
  }
  return [
    { pawn: "g6", bishop: "g7", shade: "dark" },
    { pawn: "b6", bishop: "b7", shade: "dark" },
  ];
}

function fianchetto(ctx: EvalContext): StrategyReading {
  const { pos, side } = ctx;
  const wings = fianchettoWings(side).map((wing) => {
    const pawn = pos.at(wing.pawn);
    const bishop = pos.at(wing.bishop);
    const pawnReady = pawn?.color === side && pawn.type === "p";
    const bishopHome = bishop?.color === side && bishop.type === "b";
    const colorAlive = pos
      .of(side, "b")
      .some((unit) => squareShade(unit.square) === wing.shade);
    const foundation = (pawnReady || bishopHome) && (bishopHome || colorAlive);
    const progress = bishopHome;
    const completion =
      bishopHome &&
      CENTER.some((square) => attackedSquares(pos, wing.bishop, "b", side).includes(square));
    return { wing, foundation, progress, completion, pawn: pawnReady ? pawn : null, bishop: bishopHome ? bishop : null };
  });
  const foundation = wings.some((wing) => wing.foundation);
  const progress = wings.some((wing) => wing.progress);
  const completion = wings.some((wing) => wing.completion);
  if (!foundation && !progress && !completion) {
    return empty("Neither the b-pawn nor the g-pawn has opened a fianchetto square.");
  }
  const participants = wings.flatMap((wing) => {
    if (!wing.foundation && !wing.progress) return [];
    return [wing.pawn, wing.bishop].flatMap((unit) => (unit ? [unit] : []));
  });
  const targets = wings.flatMap((wing) => {
    if (!wing.bishop) return [];
    return enemyUnitsAttackedFrom(pos, wing.bishop.square, "b", side);
  });
  const placed = wings.filter((wing) => wing.progress).map((wing) => wing.wing.bishop);
  return reading(
    { foundation, progress, completion },
    participants,
    targets,
    completion
      ? `A bishop on ${placed.join(" or ")} attacks a central square.`
      : progress
        ? `A bishop occupies ${placed.join(" or ")}, but it does not attack the center.`
        : `The b- or g-pawn has stepped forward and the bishop of that color is still on the board.`,
  );
}

function openFileControl(ctx: EvalContext): StrategyReading {
  const { pos, side } = ctx;
  const enemy = opposite(side);
  const files = [0, 1, 2, 3, 4, 5, 6, 7];
  const halfOpen = files.filter((file) => pawnsOnFile(pos, file, side).length === 0);
  const foundation = halfOpen.length > 0 && pos.of(side).some((unit) => unit.type === "r" || unit.type === "q");
  const majors = pos
    .of(side)
    .filter(
      (unit) =>
        (unit.type === "r" || unit.type === "q") && halfOpen.includes(fileIndex(unit.square)),
    );
  const progress = majors.length > 0;
  const completion = files.some((file) => {
    if (pawnsOnFile(pos, file).length > 0) return false;
    const own = pos
      .of(side)
      .filter(
        (unit) =>
          (unit.type === "r" || unit.type === "q") && fileIndex(unit.square) === file,
      ).length;
    const theirs = pos
      .of(enemy)
      .filter(
        (unit) =>
          (unit.type === "r" || unit.type === "q") && fileIndex(unit.square) === file,
      ).length;
    return own >= 1 && own > theirs;
  });
  if (!foundation && !progress && !completion) {
    return empty("Every file that contains a friendly major piece also contains a friendly pawn.");
  }
  const targets = majors.flatMap((unit) => enemyUnitsAttackedBy(pos, unit));
  const openNames = files
    .filter((file) => pawnsOnFile(pos, file).length === 0)
    .map((file) => "abcdefgh"[file])
    .join(", ");
  return reading(
    { foundation, progress, completion },
    majors.length > 0 ? majors : pos.of(side).filter((unit) => unit.type === "r" || unit.type === "q"),
    targets,
    completion
      ? `A fully open file (${openNames || "none"}) holds more friendly major pieces than opposing ones.`
      : progress
        ? `A rook or the queen has reached a file without a friendly pawn.`
        : `A file is free of friendly pawns, and a rook or queen is still on the board.`,
  );
}

function seventhRank(ctx: EvalContext): StrategyReading {
  const { pos, side } = ctx;
  const invasion = side === "w" ? 6 : 1;
  const majors = pos.of(side).filter((unit) => unit.type === "r" || unit.type === "q");
  const entry = [0, 1, 2, 3, 4, 5, 6, 7].some((file) => pawnsOnFile(pos, file, side).length === 0);
  const foundation = majors.length > 0 && entry;
  const advanced = majors.filter((unit) => relativeRank(side, unit.square) >= 5);
  const progress = advanced.length > 0;
  const invaders = majors.filter((unit) => rankIndex(unit.square) === invasion);
  const completion = invaders.some((unit) => enemyUnitsAttackedBy(pos, unit).length > 0);
  if (!foundation && !progress && !completion) {
    return empty("No rook or queen has a pawn-free file toward the seventh rank.");
  }
  const targets = invaders.flatMap((unit) => enemyUnitsAttackedBy(pos, unit));
  const rankLabel = side === "w" ? "7" : "2";
  return reading(
    { foundation, progress, completion },
    progress ? advanced : majors,
    targets,
    completion
      ? `A rook or queen on rank ${rankLabel} attacks an opposing piece.`
      : progress
        ? `A major piece has reached the sixth rank or beyond, not yet the invasion rank with a target.`
        : `A major piece and a pawn-free file are available, but the piece is still in its own camp.`,
  );
}

function passedPawnAdvance(ctx: EvalContext): StrategyReading {
  const { pos, side } = ctx;
  const enemy = opposite(side);
  const passed = pos.of(side, "p").filter((pawn) => isPassedPawn(pos, pawn));
  const foundation = passed.length > 0;
  const moved = passed.filter((pawn) => relativeRank(side, pawn.square) >= 3);
  const progress = moved.length > 0;
  const deep = passed.filter(
    (pawn) =>
      relativeRank(side, pawn.square) >= 5 && unitsAttacking(pos, pawn.square, side).length > 0,
  );
  const completion = deep.length > 0;
  if (!foundation && !progress && !completion) {
    return empty("No friendly pawn is passed.");
  }
  const featured = completion ? deep : progress ? moved : passed;
  const defenders = featured.flatMap((pawn) => unitsAttacking(pos, pawn.square, side));
  const targets = featured.flatMap((pawn) => {
    const ahead = squareAt(fileIndex(pawn.square), rankIndex(pawn.square) + (side === "w" ? 1 : -1));
    const blockers = pos
      .of(enemy)
      .filter(
        (unit) =>
          fileIndex(unit.square) === fileIndex(pawn.square) &&
          relativeRank(side, unit.square) > relativeRank(side, pawn.square),
      );
    const attackers = unitsAttacking(pos, pawn.square, enemy);
    const aheadUnit = ahead ? pos.at(ahead) : null;
    return [
      ...blockers,
      ...attackers,
      ...(aheadUnit && aheadUnit.color === enemy ? [aheadUnit] : []),
    ];
  });
  return reading(
    { foundation, progress, completion },
    [...featured, ...defenders],
    targets,
    completion
      ? `${deep.map((pawn) => pawn.square).join(", ")} is passed, at least on the sixth rank, and defended.`
      : progress
        ? `A passed pawn has reached ${moved.map((pawn) => pawn.square).join(", ")}.`
        : `A passed pawn exists on ${passed.map((pawn) => pawn.square).join(", ")} and has not reached the fourth rank.`,
  );
}

function isolatedPawnAttack(ctx: EvalContext): StrategyReading {
  const { pos, side } = ctx;
  const enemy = opposite(side);
  const isolated = pos.of(enemy, "p").filter((pawn) => isIsolatedPawn(pos, pawn));
  const foundation = isolated.length > 0;
  const pressured = isolated.filter((pawn) => {
    const attacked = unitsAttacking(pos, pawn.square, side).length > 0;
    const fileMajor = pos.of(side).some(
      (unit) =>
        (unit.type === "r" || unit.type === "q") &&
        fileIndex(unit.square) === fileIndex(pawn.square),
    );
    return attacked || fileMajor;
  });
  const progress = pressured.length > 0;
  const winning = isolated.filter((pawn) => {
    const attacks = unitsAttacking(pos, pawn.square, side).length;
    const defenses = unitsAttacking(pos, pawn.square, enemy).length;
    return attacks > defenses;
  });
  const completion = winning.length > 0;
  if (!foundation && !progress && !completion) {
    return empty("The opponent has no isolated pawn.");
  }
  const focus = completion ? winning : progress ? pressured : isolated;
  const participants = focus.flatMap((pawn) => [
    ...unitsAttacking(pos, pawn.square, side),
    ...pos
      .of(side)
      .filter(
        (unit) =>
          (unit.type === "r" || unit.type === "q") &&
          fileIndex(unit.square) === fileIndex(pawn.square),
      ),
  ]);
  const targets = focus.flatMap((pawn) => [
    pawn,
    ...unitsAttacking(pos, pawn.square, enemy).filter((unit) => unit.type !== "k"),
  ]);
  return reading(
    { foundation, progress, completion },
    participants,
    targets,
    completion
      ? `${winning.map((pawn) => pawn.square).join(", ")} is isolated and attacked more times than it is defended.`
      : progress
        ? `An isolated pawn on ${pressured.map((pawn) => pawn.square).join(", ")} is under fire or shares a file with a major piece.`
        : `An isolated pawn stands on ${isolated.map((pawn) => pawn.square).join(", ")}.`,
  );
}

function isBackwardPawn(ctx: EvalContext, pawn: Unit): boolean {
  const { pos, side } = ctx;
  if (pawn.type !== "p" || pawn.color === side) return false;
  const advance = squareAt(
    fileIndex(pawn.square),
    rankIndex(pawn.square) + (pawn.color === "w" ? 1 : -1),
  );
  if (!advance) return false;
  const advanceAttackedByPawn = unitsAttacking(pos, advance, side).some((unit) => unit.type === "p");
  if (!advanceAttackedByPawn) return false;
  const defendedByOwnPawn = unitsAttacking(pos, pawn.square, pawn.color).some((unit) => unit.type === "p");
  if (defendedByOwnPawn) return false;
  if (pawnsOnFile(pos, fileIndex(pawn.square), side).length > 0) return false;
  return true;
}

function backwardPawnPressure(ctx: EvalContext): StrategyReading {
  const { pos, side } = ctx;
  const enemy = opposite(side);
  const backward = pos.of(enemy, "p").filter((pawn) => isBackwardPawn(ctx, pawn));
  const foundation = backward.length > 0;
  const filed = backward.filter((pawn) =>
    pos.of(side).some(
      (unit) =>
        (unit.type === "r" || unit.type === "q") &&
        fileIndex(unit.square) === fileIndex(pawn.square),
    ),
  );
  const progress = filed.length > 0;
  const completed = filed.filter((pawn) => {
    const advance = squareAt(
      fileIndex(pawn.square),
      rankIndex(pawn.square) + (pawn.color === "w" ? 1 : -1),
    );
    if (!advance) return false;
    const blockaded =
      pos.at(advance)?.color === side || unitsAttacking(pos, advance, side).length > 0;
    return blockaded;
  });
  const completion = completed.length > 0;
  if (!foundation && !progress && !completion) {
    return empty("The opponent has no backward pawn on a half-open file.");
  }
  const focus = completion ? completed : progress ? filed : backward;
  const participants = focus.flatMap((pawn) => {
    const advance = squareAt(
      fileIndex(pawn.square),
      rankIndex(pawn.square) + (pawn.color === "w" ? 1 : -1),
    );
    return [
      ...pos
        .of(side)
        .filter(
          (unit) =>
            (unit.type === "r" || unit.type === "q") &&
            fileIndex(unit.square) === fileIndex(pawn.square),
        ),
      ...(advance ? unitsAttacking(pos, advance, side) : []),
    ];
  });
  return reading(
    { foundation, progress, completion },
    participants,
    focus.flatMap((pawn) => [pawn, ...unitsAttacking(pos, pawn.square, enemy)]),
    completion
      ? `A major piece bears on ${completed.map((pawn) => pawn.square).join(", ")} and the pawn's advance square is controlled.`
      : progress
        ? `A rook or queen shares the file of the backward pawn on ${filed.map((pawn) => pawn.square).join(", ")}.`
        : `A backward pawn stands on ${backward.map((pawn) => pawn.square).join(", ")}.`,
  );
}

function enemyPawnCanStillAttack(ctx: EvalContext, square: Square): boolean {
  const enemy = opposite(ctx.side);
  const targetRel = relativeRank(enemy, square);
  return ctx.pos.of(enemy, "p").some((pawn) => {
    if (Math.abs(fileIndex(pawn.square) - fileIndex(square)) !== 1) return false;
    return relativeRank(enemy, pawn.square) < targetRel;
  });
}

function outpostSquares(ctx: EvalContext): Square[] {
  const { pos, side } = ctx;
  const squares: Square[] = [];
  for (let file = 0; file < 8; file += 1) {
    for (let rank = 0; rank < 8; rank += 1) {
      const square = squareAt(file, rank);
      if (!square) continue;
      const rel = relativeRank(side, square);
      if (rel < 3 || rel > 6) continue;
      const occupant = pos.at(square);
      if (occupant && !(occupant.color === side && occupant.type === "n")) continue;
      if (!unitsAttacking(pos, square, side).some((unit) => unit.type === "p")) continue;
      if (enemyPawnCanStillAttack(ctx, square)) continue;
      squares.push(square);
    }
  }
  return squares;
}

function knightOutpost(ctx: EvalContext): StrategyReading {
  const { pos, side } = ctx;
  const squares = outpostSquares(ctx);
  const foundation = squares.length > 0;
  const knights = pos.of(side, "n");
  const occupying = knights.filter((knight) => squares.includes(knight.square));
  const eyeing = knights.filter((knight) =>
    squares.some((square) => attackedSquares(pos, knight.square, "n", side).includes(square)),
  );
  const progress = occupying.length > 0 || eyeing.length > 0;
  const completion = occupying.length > 0;
  if (!foundation && !progress && !completion) {
    return empty("No pawn-defended square in the opponent's half is safe from enemy pawns.");
  }
  const relevant = completion ? occupying : progress ? [...occupying, ...eyeing] : [];
  const defenders = (completion ? occupying.map((knight) => knight.square) : squares).flatMap((square) =>
    unitsAttacking(pos, square, side).filter((unit) => unit.type === "p"),
  );
  const targets = occupying.flatMap((knight) => enemyUnitsAttackedBy(pos, knight));
  const vacantTargets = squares.flatMap((square) => {
    const occupant = pos.at(square);
    return occupant && occupant.color !== side ? [occupant] : [];
  });
  return reading(
    { foundation, progress, completion },
    [...relevant, ...defenders],
    [...targets, ...vacantTargets],
    completion
      ? `A knight occupies the outpost on ${occupying.map((knight) => knight.square).join(", ")}.`
      : progress
        ? `A knight attacks an outpost square (${squares.join(", ")}).`
        : `An outpost square exists on ${squares.join(", ")}, with no knight attending it.`,
  );
}

function bishopPair(ctx: EvalContext): StrategyReading {
  const { pos, side } = ctx;
  const enemy = opposite(side);
  const bishops = pos.of(side, "b");
  const enemyBishops = pos.of(enemy, "b").length;
  const foundation = bishops.length === 2 && enemyBishops < 2;
  const homeRankIndex = homeRank(side);
  const developed = bishops.filter((bishop) => rankIndex(bishop.square) !== homeRankIndex);
  const progress = foundation && developed.length >= 1;
  const openCenter = [2, 3, 4, 5].some((file) => pawnsOnFile(pos, file).length === 0);
  const longRange = bishops.some((bishop) => clearTravel(pos, bishop) >= 3);
  const completion = progress && openCenter && longRange;
  if (!foundation && !progress && !completion) {
    return empty("The bishop pair is not ahead of the opponent's bishops.");
  }
  const targets = bishops.flatMap((bishop) => enemyUnitsAttackedBy(pos, bishop));
  const knights = pos.of(enemy, "n");
  return reading(
    { foundation, progress, completion },
    bishops,
    [...targets, ...knights.filter((knight) => bishops.some((bishop) => 
      attackedSquares(pos, bishop.square, "b", side).includes(knight.square),
    ))],
    completion
      ? `Both bishops remain, a central file is open, and a bishop has three clear squares of travel.`
      : progress
        ? `One bishop has left the back rank. The position is not yet open enough for the pair.`
        : `Both bishops remain while the opponent has fewer, and both are still on the back rank.`,
  );
}

interface Lineup {
  slider: Unit;
  front: Unit;
  rear: Unit;
  clear: boolean;
}

function opposingLineups(ctx: EvalContext, kind: "pin" | "skewer"): Lineup[] {
  const { pos, side } = ctx;
  const enemy = opposite(side);
  const lineups: Lineup[] = [];
  const sliders = pos.of(side).filter((unit) => unit.type === "b" || unit.type === "r" || unit.type === "q");
  for (const slider of sliders) {
    for (const [df, dr] of sliderDirs(slider.type)) {
      const ray = rayUnits(pos, slider.square, df, dr);
      for (let frontIndex = 0; frontIndex < ray.length; frontIndex += 1) {
        for (let rearIndex = frontIndex + 1; rearIndex < ray.length; rearIndex += 1) {
          const front = ray[frontIndex];
          const rear = ray[rearIndex];
          if (front.color !== enemy || rear.color !== enemy) continue;
          if (rearIndex - frontIndex !== 1) continue;
          if (frontIndex > 1) continue;
          if (frontIndex === 1 && ray[0].color === side) continue;
          const frontValue = compareValue(front.type);
          const rearValue = compareValue(rear.type);
          if (kind === "pin" && !(rear.type === "k" || rearValue > frontValue)) continue;
          if (kind === "skewer" && frontValue < rearValue) continue;
          lineups.push({ slider, front, rear, clear: frontIndex === 0 });
        }
      }
    }
  }
  return lineups;
}

function pin(ctx: EvalContext): StrategyReading {
  const { pos, side } = ctx;
  const lineups = opposingLineups(ctx, "pin");
  const foundation = lineups.length > 0;
  const clear = lineups.filter((lineup) => lineup.clear);
  const progress = clear.length > 0;
  const completion = clear.some(
    (lineup) =>
      lineup.rear.type === "k" ||
      unitsAttacking(pos, lineup.front.square, side).some((unit) => unit.square !== lineup.slider.square),
  );
  if (!foundation && !progress && !completion) {
    return empty("No slider is aligned with a lesser piece and a heavier piece behind it.");
  }
  const relevant = progress ? clear : lineups;
  const extra = clear.flatMap((lineup) =>
    unitsAttacking(pos, lineup.front.square, side).filter((unit) => unit.square !== lineup.slider.square),
  );
  return reading(
    { foundation, progress, completion },
    [...relevant.map((lineup) => lineup.slider), ...extra],
    relevant.map((lineup) => lineup.front),
    completion
      ? `The pin on ${clear.map((lineup) => lineup.front.square).join(", ")} is absolute or backed by a second attacker.`
      : progress
        ? `A slider already attacks the front piece of a pin alignment.`
        : `A pin alignment exists, with one piece still blocking the slider.`,
  );
}

function moveLeavesLine(slider: Square, screen: Square, destination: Square): boolean {
  const screenLine = pieceCountBetween(
    { units: [], at: () => null, of: () => [], king: () => null },
    slider,
    screen,
  );
  const destLine = pieceCountBetween(
    { units: [], at: () => null, of: () => [], king: () => null },
    slider,
    destination,
  );
  if (screenLine === null || destLine === null) return true;
  const sameDirection =
    Math.sign(fileIndex(screen) - fileIndex(slider)) === Math.sign(fileIndex(destination) - fileIndex(slider)) &&
    Math.sign(rankIndex(screen) - rankIndex(slider)) === Math.sign(rankIndex(destination) - rankIndex(slider));
  return !sameDirection;
}

function discoveredAttack(ctx: EvalContext): StrategyReading {
  const { pos, chess, side } = ctx;
  const enemy = opposite(side);
  const discoveries: { slider: Unit; screen: Unit; target: Unit }[] = [];
  for (const slider of pos.of(side)) {
    if (slider.type !== "b" && slider.type !== "r" && slider.type !== "q") continue;
    for (const [df, dr] of sliderDirs(slider.type)) {
      const ray = rayUnits(pos, slider.square, df, dr);
      if (ray.length < 2) continue;
      const [screen, target] = ray;
      if (screen.color !== side || target.color !== enemy || target.type === "p") continue;
      discoveries.push({ slider, screen, target });
    }
  }
  const foundation = discoveries.length > 0;
  if (!foundation) {
    return empty("No friendly piece stands between a slider and an opposing piece other than a pawn.");
  }
  const legal = movesFor(chess, side);
  let progress = false;
  let completion = false;
  const readyScreens = new Set<string>();
  const forcingScreens = new Set<string>();
  for (const discovery of discoveries) {
    const screenMoves = legal.filter((move) => move.from === discovery.screen.square);
    for (const move of screenMoves) {
      if (!moveLeavesLine(discovery.slider.square, discovery.screen.square, move.to)) continue;
      progress = true;
      readyScreens.add(discovery.screen.square);
      const next = cloneChess(chess.turn() === side ? chess : flipped(chess, side));
      try {
        next.move({ from: move.from, to: move.to, promotion: move.promotion });
      } catch {
        continue;
      }
      const slider = readUnit(next, discovery.slider.square);
      if (!slider) continue;
      const attacksTarget = attackedSquares(
        readPosition(next),
        slider.square,
        slider.type,
        side,
      ).includes(discovery.target.square);
      if (attacksTarget && (move.isCapture() || next.isCheck())) {
        completion = true;
        forcingScreens.add(discovery.screen.square);
      }
    }
  }
  const participants = discoveries
    .filter((discovery) => (completion ? forcingScreens.has(discovery.screen.square) : progress ? readyScreens.has(discovery.screen.square) : true))
    .flatMap((discovery) => [discovery.slider, discovery.screen]);
  const targets = discoveries
    .filter((discovery) => (completion ? forcingScreens.has(discovery.screen.square) : true))
    .map((discovery) => discovery.target);
  return reading(
    { foundation, progress, completion },
    participants,
    targets,
    completion
      ? `Moving the screening piece captures or gives check and uncovers the slider.`
      : progress
        ? `The screening piece has a legal move off the line, but it does not yet capture or check.`
        : `A friendly piece blocks a slider's line to an opposing piece and cannot move off that line.`,
  );
}

function flipped(chess: EvalContext["chess"], side: Color) {
  const copy = cloneChess(chess);
  copy.setTurn(side);
  return copy;
}

function readUnit(chess: EvalContext["chess"], square: Square): Unit | null {
  const piece = chess.get(square);
  if (!piece) return null;
  return { square, type: piece.type, color: piece.color };
}

function forkPairQualifies(forkerType: PieceSymbol, targets: Unit[], pos: EvalContext["pos"], color: Color, strict: boolean): boolean {
  if (targets.length < 2) return false;
  const ranked = [...targets].sort((a, b) => compareValue(b.type) - compareValue(a.type));
  const pair = ranked.slice(0, 2);
  if (!strict && (forkerType === "n" || forkerType === "p")) return true;
  const forkerValue = forkerType === "k" ? compareValue("k") : materialValue(forkerType as Exclude<PieceSymbol, "k">);
  return pair.every((target) => {
    if (target.type === "k") return true;
    if (compareValue(target.type) >= forkerValue) return true;
    return unitsAttacking(pos, target.square, opposite(color)).length === 0;
  });
}

function forkTargetsFrom(pos: EvalContext["pos"], square: Square, type: PieceSymbol, color: Color): Unit[] {
  return enemyUnitsAttackedFrom(pos, square, type, color).filter((unit) => unit.type !== "p");
}

function fork(ctx: EvalContext): StrategyReading {
  const { pos, chess, side } = ctx;
  const movers = pos.of(side).filter((unit) => unit.type !== "k");
  let foundation = false;
  let progress = false;
  let completion = false;
  const participants: Unit[] = [];
  const targets: Unit[] = [];

  for (const mover of movers) {
    const current = forkTargetsFrom(pos, mover.square, mover.type, side);
    const currentStrict = forkPairQualifies(mover.type, current, pos, side, true);
    const currentLoose = forkPairQualifies(mover.type, current, pos, side, false);
    if (currentLoose) {
      foundation = true;
      progress = true;
      participants.push(mover);
      targets.push(...current);
      if (currentStrict) completion = true;
    }
  }

  if (!completion) {
    for (const move of movesFor(chess, side)) {
      const type = (move.promotion ?? move.piece) as PieceSymbol;
      if (type === "k") continue;
      const landed = forkTargetsFrom(pos, move.to, type, side).filter((unit) => unit.square !== move.to);
      if (!forkPairQualifies(type, landed, pos, side, type === "n" || type === "p" ? false : true)) {
        continue;
      }
      foundation = true;
      const mover = pos.at(move.from);
      if (mover) participants.push(mover);
      targets.push(...landed);
    }
  }

  if (!foundation && !progress && !completion) {
    return empty("No piece attacks, or can move to a square attacking, two opposing pieces above pawn value.");
  }
  return reading(
    { foundation, progress, completion },
    participants,
    targets,
    completion
      ? `A piece currently attacks two targets that are undefended, royal, or at least as valuable as the attacker.`
      : progress
        ? `A knight or pawn currently attacks two pieces, but the fork is not yet a material win.`
        : `A legal move reaches a square that forks two qualifying pieces.`,
  );
}

function skewer(ctx: EvalContext): StrategyReading {
  const { pos, side } = ctx;
  const lineups = opposingLineups(ctx, "skewer");
  const foundation = lineups.length > 0;
  const clear = lineups.filter((lineup) => {
    if (!lineup.clear) return false;
    return lineup.front.type === "k" || compareValue(lineup.front.type) > compareValue(lineup.rear.type);
  });
  const progress = clear.length > 0;
  const completion = clear.some((lineup) => {
    if (lineup.front.type === "k") return true;
    const sliderValue = materialValue(lineup.slider.type as Exclude<PieceSymbol, "k">);
    const frontHigher = compareValue(lineup.front.type) > sliderValue;
    const rearLoose = unitsAttacking(pos, lineup.rear.square, opposite(side)).length === 0;
    const rearWorth = compareValue(lineup.rear.type) >= sliderValue;
    return frontHigher && (rearLoose || rearWorth);
  });
  if (!foundation && !progress && !completion) {
    return empty("No slider is aligned with a heavier piece in front of a lighter one.");
  }
  const relevant = progress ? clear : lineups;
  return reading(
    { foundation, progress, completion },
    relevant.map((lineup) => lineup.slider),
    relevant.flatMap((lineup) => [lineup.front, lineup.rear]),
    completion
      ? `The skewer on ${clear.map((lineup) => `${lineup.front.square} in front of ${lineup.rear.square}`).join("; ")} is forcing.`
      : progress
        ? `A slider attacks the front piece of a skewer, but the gain is not yet forcing.`
        : `A skewer alignment exists with one piece still in the way.`,
  );
}

function originalMajorSquare(unit: Unit): boolean {
  if (unit.type === "q") return unit.square === (unit.color === "w" ? "d1" : "d8");
  if (unit.type === "r") {
    return unit.color === "w"
      ? unit.square === "a1" || unit.square === "h1"
      : unit.square === "a8" || unit.square === "h8";
  }
  return false;
}

function majorBattery(ctx: EvalContext): StrategyReading {
  const { pos, side } = ctx;
  const majors = pos.of(side).filter((unit) => unit.type === "r" || unit.type === "q");
  const pairs: { a: Unit; b: Unit; between: number }[] = [];
  for (let i = 0; i < majors.length; i += 1) {
    for (let j = i + 1; j < majors.length; j += 1) {
      const a = majors[i];
      const b = majors[j];
      if (fileIndex(a.square) !== fileIndex(b.square) && rankIndex(a.square) !== rankIndex(b.square)) {
        continue;
      }
      const between = pieceCountBetween(pos, a.square, b.square);
      if (between === null) continue;
      pairs.push({ a, b, between });
    }
  }
  const developed = pairs.filter((pair) => !originalMajorSquare(pair.a) || !originalMajorSquare(pair.b));
  const foundation = developed.some((pair) => pair.between <= 2);
  const progressPairs = developed.filter((pair) => pair.between <= 1);
  const progress = progressPairs.length > 0;
  const completed = developed.filter((pair) => {
    if (pair.between !== 0) return false;
    const shareFile = fileIndex(pair.a.square) === fileIndex(pair.b.square);
    const halfOpen = shareFile && pawnsOnFile(pos, fileIndex(pair.a.square), side).length === 0;
    const bears = [pair.a, pair.b].some((piece) =>
      enemyUnitsAttackedBy(pos, piece).some((target) =>
        shareFile
          ? fileIndex(target.square) === fileIndex(piece.square)
          : rankIndex(target.square) === rankIndex(piece.square),
      ),
    );
    return halfOpen || bears;
  });
  const completion = completed.length > 0;
  if (!foundation && !progress && !completion) {
    return empty("The rooks and queen are not gathered on a shared file or rank.");
  }
  const chosen = completion ? completed : progress ? progressPairs : developed.filter((pair) => pair.between <= 2);
  const participants = chosen.flatMap((pair) => [pair.a, pair.b]);
  const targets = participants.flatMap((piece) => enemyUnitsAttackedBy(pos, piece));
  return reading(
    { foundation, progress, completion },
    participants,
    targets,
    completion
      ? `Two major pieces are connected and the line is half-open or bears on an opposing piece.`
      : progress
        ? `Two major pieces share a line with at most one piece between them.`
        : `Two developed major pieces share a line with two pieces still between them.`,
  );
}

function promotion(ctx: EvalContext): StrategyReading {
  const { pos, chess, side } = ctx;
  const advanced = pos.of(side, "p").filter((pawn) => relativeRank(side, pawn.square) >= 4);
  const foundation = advanced.length > 0;
  const deep = advanced.filter((pawn) => relativeRank(side, pawn.square) >= 5);
  const progress = deep.length > 0;
  const promotingMoves = movesFor(chess, side).filter((move) => move.piece === "p" && move.promotion);
  const completion = promotingMoves.length > 0;
  if (!foundation && !progress && !completion) {
    return empty("No pawn has reached the opponent's half of the board.");
  }
  const promotingSquares = new Set(promotingMoves.map((move) => move.from));
  const featured = completion
    ? advanced.filter((pawn) => promotingSquares.has(pawn.square))
    : progress
      ? deep
      : advanced;
  const defenders = featured.flatMap((pawn) => unitsAttacking(pos, pawn.square, side));
  const targets = featured.flatMap((pawn) => {
    const aheadRank = rankIndex(pawn.square) + (side === "w" ? 1 : -1);
    const ahead = squareAt(fileIndex(pawn.square), aheadRank);
    const aheadUnit = ahead ? pos.at(ahead) : null;
    const attackers = unitsAttacking(pos, pawn.square, opposite(side));
    const diagonal = attackedSquares(pos, pawn.square, "p", side)
      .map((square) => pos.at(square))
      .filter((unit): unit is Unit => !!unit && unit.color !== side);
    return [
      ...attackers,
      ...diagonal,
      ...(aheadUnit && aheadUnit.color !== side ? [aheadUnit] : []),
    ];
  });
  return reading(
    { foundation, progress, completion },
    [...featured, ...defenders],
    targets,
    completion
      ? `A pawn on ${[...promotingSquares].join(", ")} has a legal promotion move.`
      : progress
        ? `A pawn has reached the sixth rank or beyond.`
        : `A pawn has entered the opponent's half.`,
  );
}

function define(
  id: string,
  name: string,
  intent: string,
  foundation: string,
  progress: string,
  completion: string,
  captureLink: string,
  read: (ctx: EvalContext) => StrategyReading,
): StrategyDefinition {
  return { id, name, intent, foundation, progress, completion, captureLink, read };
}

export const STRATEGIES: StrategyDefinition[] = [
  define(
    "central-pawn-occupation",
    "Central pawn occupation",
    "Occupy d4 and e4, or d5 and e5, with pawns that the rest of the army defends.",
    "At least one friendly pawn stands on the d-file or the e-file.",
    "A friendly pawn occupies the side's central squares: d4 or e4 for White, d5 or e5 for Black.",
    "Friendly pawns occupy both of those squares, and each is geometrically defended.",
    "The capturer is a central pawn or a piece defending one. The victim stands on c4–f5 or is a pawn attacking a friendly central pawn.",
    centralPawnOccupation,
  ),
  define(
    "central-pawn-break",
    "Central pawn break",
    "Challenge an opposing central pawn with a c-, d-, e-, or f-pawn capture.",
    "The opponent has a pawn on d4, d5, e4, or e5, and a friendly pawn remains on the c-, d-, e-, or f-file.",
    "A friendly pawn geometrically attacks an opposing pawn on one of those four squares.",
    "The recorded window of the previous four turns, plus the current turn when it has been played, contains a friendly pawn capturing a pawn on the c-, d-, e-, or f-file.",
    "The capturer is a c–f pawn. The victim is an opposing central pawn that pawn attacks.",
    centralPawnBreak,
  ),
  define(
    "kingside-pawn-storm",
    "Kingside pawn storm",
    "Advance the f-, g-, and h-pawns against the enemy king while the friendly king stays off that wing.",
    "At least two pawns remain on f–h, at least one has left its starting square, and the king is not on f, g, or h.",
    "A pawn on f, g, or h has relative rank 2 or more, and the king is still off f–h.",
    "At least two of those pawns have relative rank 3 or more, and the king is on files a–d.",
    "The capturer is a storm pawn or a piece attacking the f–h wing. The victim stands on f, g, or h.",
    kingsidePawnStorm,
  ),
  define(
    "queenside-minority-attack",
    "Queenside minority attack",
    "Advance a smaller queenside pawn mass to leave the opponent with an isolated or doubled pawn.",
    "On files a–c the opponent has strictly more pawns, the side has at least one, and the opponent has a pawn on a–d.",
    "A friendly pawn on a, b, or c has relative rank 2 or more.",
    "A friendly pawn on a–c has relative rank 3 or more, and an opposing pawn on a–d is isolated or doubled.",
    "The capturer is an a–c pawn or a major piece on a–d. The victim is an opposing pawn on a–d.",
    queensideMinority,
  ),
  define(
    "king-shelter",
    "King shelter",
    "Castle and keep the three pawns in front of the king on their original squares.",
    "The king is on e1 or e8 with at least one castling right, or the king is already on c1, g1, c8, or g8.",
    "The king is on a castled square, or at most one piece remains between the king and a rook that still has castling rights.",
    "The king is on g1, g8, c1, or c8, is not attacked, and the three shelter pawns on that wing are unmoved.",
    "The capturer is the king, a shelter pawn, or a back-rank rook. The victim attacks the king or a shelter pawn.",
    kingShelter,
  ),
  define(
    "opposite-side-castling-attack",
    "Opposite-side castling attack",
    "Storm the wing where the enemy king lives after the kings have castled on opposite sides.",
    "The friendly king is on the home rank on files a–c or f–h, and the enemy king is on the other wing (files f–h or a–c).",
    "A friendly pawn on the enemy king's wing has relative rank 2 or more.",
    "One of those pawns has relative rank 3 or more, and a friendly rook or queen stands on a half-open file of that wing.",
    "The capturer is a wing pawn or a major piece on that wing. The victim stands on the enemy king's wing.",
    oppositeCastlingAttack,
  ),
  define(
    "fianchetto",
    "Fianchetto",
    "Place a bishop on the long diagonal behind a one-step b- or g-pawn and aim it at the center.",
    "The b- or g-pawn has moved exactly one square, or the bishop is already on b2, g2, b7, or g7, and that color of bishop still exists.",
    "The bishop occupies the fianchetto square.",
    "That bishop geometrically attacks d4, d5, e4, or e5.",
    "The capturer is the fianchetto bishop or the pawn that opened its diagonal. The victim is a piece the bishop attacks.",
    fianchetto,
  ),
  define(
    "open-file-control",
    "Open-file control",
    "Put a rook or queen on a file with no friendly pawn, and own a fully open file.",
    "A file has no friendly pawn, and a friendly rook or queen is still on the board.",
    "A friendly rook or queen stands on a file with no friendly pawn.",
    "A fully open file contains more friendly rooks and queens than opposing ones.",
    "The capturer is a rook or queen on a half-open file. The victim is a piece that major piece attacks.",
    openFileControl,
  ),
  define(
    "seventh-rank-invasion",
    "Seventh-rank invasion",
    "Bring a rook or queen to the opponent's second rank and attack something there.",
    "A friendly rook or queen exists, and at least one file has no friendly pawn.",
    "A friendly rook or queen has relative rank 5 or more.",
    "A friendly rook or queen is on the opponent's second rank and geometrically attacks an opposing unit.",
    "The capturer is a major piece on the sixth rank or beyond. The victim is a piece that invader attacks.",
    seventhRank,
  ),
  define(
    "passed-pawn-advance",
    "Passed-pawn advance",
    "Create a passed pawn, escort it past the midline, and defend it on the sixth rank or beyond.",
    "A friendly pawn has no opposing pawn on its file or an adjacent file in front of it.",
    "That passed pawn has relative rank 3 or more.",
    "A passed pawn has relative rank 5 or more and is geometrically defended.",
    "The capturer is the passed pawn or a piece defending it. The victim blocks it, attacks it, or stands on the square in front of it.",
    passedPawnAdvance,
  ),
  define(
    "isolated-pawn-attack",
    "Isolated-pawn attack",
    "Fix an opposing pawn that has no neighbor, then attack it more times than it is defended.",
    "The opponent has a pawn with no friendly pawn on an adjacent file.",
    "A friendly unit geometrically attacks that pawn, or a friendly rook or queen shares its file.",
    "The isolated pawn is geometrically attacked more times than it is defended.",
    "The capturer attacks the isolated pawn or shares its file as a rook or queen. The victim is the isolated pawn or a piece defending it.",
    isolatedPawnAttack,
  ),
  define(
    "backward-pawn-pressure",
    "Backward-pawn pressure",
    "Restrain a pawn whose advance square is hit by a friendly pawn, then line a major piece up against it.",
    "An opposing pawn's advance square is attacked by a friendly pawn, no opposing pawn defends it, and its file has no friendly pawn.",
    "A friendly rook or queen shares that pawn's file.",
    "The advance square is occupied or attacked by the side, and the major piece remains on the file.",
    "The capturer is the major piece or a unit controlling the advance square. The victim is the backward pawn or a piece defending it.",
    backwardPawnPressure,
  ),
  define(
    "knight-outpost",
    "Knight outpost",
    "Plant a knight on a square in the opponent's half that a friendly pawn defends and no enemy pawn can attack.",
    "Such a square exists on relative ranks 3 through 6. It is empty or already occupied by a friendly knight.",
    "A friendly knight occupies the square or geometrically attacks it.",
    "A friendly knight occupies the square.",
    "The capturer is the knight or the pawn that defends the outpost. The victim is a piece the occupying knight attacks.",
    knightOutpost,
  ),
  define(
    "bishop-pair",
    "Bishop pair",
    "Keep both bishops against fewer opposing bishops and open a central file for them.",
    "The side has two bishops and the opponent has fewer than two.",
    "At least one friendly bishop has left the back rank.",
    "A file from c through f is empty of pawns, and a friendly bishop has at least three squares of clear travel.",
    "The capturer is one of the two bishops. The victim is a piece that bishop attacks.",
    bishopPair,
  ),
  define(
    "pin",
    "Pin",
    "Align a bishop, rook, or queen against a piece that cannot step aside without exposing a heavier piece or the king.",
    "A friendly slider, an opposing front piece, and a heavier opposing rear piece or the king share a line, with at most one non-friendly piece between the slider and the front piece and nothing between the two opposing pieces.",
    "Nothing remains between the slider and the front piece.",
    "The line is clear and either the rear piece is the king or a second friendly unit also attacks the front piece.",
    "The capturer is the pinning slider or the second attacker. The victim is the pinned piece.",
    pin,
  ),
  define(
    "discovered-attack",
    "Discovered attack",
    "Move a piece off a slider's line so the slider's attack arrives as the mover captures or gives check.",
    "Exactly one friendly piece stands between a friendly slider and an opposing piece other than a pawn.",
    "That screening piece has a legal move off the line.",
    "One of those moves captures or gives check, and the slider still attacks the original target afterward.",
    "The capturer is the slider or the screening piece. The victim is the discovered target.",
    discoveredAttack,
  ),
  define(
    "fork",
    "Fork",
    "Attack two pieces of at least knight value with one lesser piece, or reach the square that does it.",
    "A legal move, or the piece's current square, attacks two opposing units other than pawns. Slider forks count only when both targets are royal, undefended, or at least as valuable as the slider.",
    "A knight or pawn currently attacks two such pieces, or a slider currently has a qualifying pair.",
    "Two current targets are each a king, geometrically undefended, or worth at least as much as the forker.",
    "The capturer is the forking piece. The victim is one of the forked pieces.",
    fork,
  ),
  define(
    "skewer",
    "Skewer",
    "Attack a more valuable piece that must step off a line and leave a lesser piece behind it.",
    "A friendly slider is aligned with two opposing pieces, the nearer one at least as valuable as the farther one, with at most one non-friendly blocker in front.",
    "The line to the front piece is clear, and the front piece is the king or strictly more valuable than the piece behind it.",
    "The front piece is the king, or it is worth more than the slider and the rear piece is undefended or worth at least the slider.",
    "The capturer is the slider. The victim is the front piece or the piece behind it.",
    skewer,
  ),
  define(
    "major-piece-battery",
    "Major-piece battery",
    "Double rooks, or a rook and queen, on a file or rank that is open or aimed at a target.",
    "Two friendly rooks or queens share a file or rank, at most two pieces stand between them, and at least one has left its original square.",
    "At most one piece stands between them.",
    "Nothing stands between them, and they share a half-open file or one of them attacks an opposing piece along the line.",
    "The capturer is one of the battery pieces. The victim is a piece that battery piece attacks.",
    majorBattery,
  ),
  define(
    "promotion",
    "Promotion",
    "Escort a pawn into the opponent's half and give it a legal move onto the last rank.",
    "A friendly pawn has relative rank 4 or more.",
    "A friendly pawn has relative rank 5 or more.",
    "A friendly pawn has a legal move that promotes.",
    "The capturer is the advanced pawn or a piece defending it. The victim attacks that pawn, stands in front of it, or can be taken by it.",
    promotion,
  ),
];

export function strategyById(id: string): StrategyDefinition {
  const found = STRATEGIES.find((strategy) => strategy.id === id);
  if (!found) throw new Error(`Unknown strategy ${id}`);
  return found;
}
