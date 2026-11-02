import type { Color, PieceSymbol, Square } from "chess.js";

export type { Color, PieceSymbol, Square };

/** A piece on a square. Kings use a comparison sentinel and are never priced as captures. */
export interface Unit {
  square: Square;
  type: PieceSymbol;
  color: Color;
}

export const FILES = "abcdefgh";

/**
 * Centipawn-style piece values used for every capture.
 * Bishop is valued slightly above knight so an otherwise equal minor-piece trade is not a tie.
 */
export const PIECE_VALUE = {
  p: 100,
  n: 320,
  b: 330,
  r: 500,
  q: 900,
} as const;

/** Used only to compare kings with other pieces inside fork and skewer rules. */
export const KING_COMPARE = 10_000;

/** Added to expected gain, or to potential loss, when a capture in the exchange is checkmate. */
export const MATE_BONUS = 100_000;

export function opposite(color: Color): Color {
  return color === "w" ? "b" : "w";
}

export function materialValue(type: Exclude<PieceSymbol, "k">): number {
  return PIECE_VALUE[type];
}

export function compareValue(type: PieceSymbol): number {
  return type === "k" ? KING_COMPARE : PIECE_VALUE[type];
}

export function fileIndex(square: Square): number {
  return square.charCodeAt(0) - 97;
}

export function rankIndex(square: Square): number {
  return square.charCodeAt(1) - 49;
}

export function squareAt(file: number, rank: number): Square | null {
  if (file < 0 || file > 7 || rank < 0 || rank > 7) return null;
  return `${FILES[file]}${rank + 1}` as Square;
}

export function forward(color: Color): number {
  return color === "w" ? 1 : -1;
}

export function homeRank(color: Color): number {
  return color === "w" ? 0 : 7;
}

/** 0 on the player's own back rank, 7 on the promotion rank. */
export function relativeRank(color: Color, square: Square): number {
  const rank = rankIndex(square);
  return color === "w" ? rank : 7 - rank;
}

export function squareShade(square: Square): "light" | "dark" {
  return (fileIndex(square) + rankIndex(square)) % 2 === 0 ? "dark" : "light";
}

export function pieceName(type: PieceSymbol): string {
  switch (type) {
    case "p":
      return "pawn";
    case "n":
      return "knight";
    case "b":
      return "bishop";
    case "r":
      return "rook";
    case "q":
      return "queen";
    case "k":
      return "king";
  }
}

export function sideName(color: Color): string {
  return color === "w" ? "White" : "Black";
}

export function uniqUnits(units: Unit[]): Unit[] {
  const seen = new Set<string>();
  const out: Unit[] = [];
  for (const unit of units) {
    const key = `${unit.color}${unit.type}${unit.square}`;
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(unit);
  }
  return out;
}

const KNIGHT_DELTAS: ReadonlyArray<readonly [number, number]> = [
  [1, 2],
  [2, 1],
  [-1, 2],
  [-2, 1],
  [1, -2],
  [2, -1],
  [-1, -2],
  [-2, -1],
];

const KING_DELTAS: ReadonlyArray<readonly [number, number]> = [
  [1, 0],
  [-1, 0],
  [0, 1],
  [0, -1],
  [1, 1],
  [1, -1],
  [-1, 1],
  [-1, -1],
];

const BISHOP_DIRS: ReadonlyArray<readonly [number, number]> = [
  [1, 1],
  [1, -1],
  [-1, 1],
  [-1, -1],
];

const ROOK_DIRS: ReadonlyArray<readonly [number, number]> = [
  [1, 0],
  [-1, 0],
  [0, 1],
  [0, -1],
];

export function sliderDirs(type: PieceSymbol): ReadonlyArray<readonly [number, number]> {
  if (type === "b") return BISHOP_DIRS;
  if (type === "r") return ROOK_DIRS;
  if (type === "q") return [...BISHOP_DIRS, ...ROOK_DIRS];
  return [];
}

export interface Position {
  units: Unit[];
  at(square: Square): Unit | null;
  of(color: Color, type?: PieceSymbol): Unit[];
  king(color: Color): Unit | null;
}

/**
 * Geometric attacks. Pins and turn order are ignored, so a piece with no legal
 * move can still attack a square and still count toward a strategy.
 */
export function attackedSquares(
  pos: Position,
  from: Square,
  type: PieceSymbol,
  color: Color,
): Square[] {
  const file = fileIndex(from);
  const rank = rankIndex(from);
  const out: Square[] = [];
  const push = (nextFile: number, nextRank: number) => {
    const square = squareAt(nextFile, nextRank);
    if (square) out.push(square);
  };

  if (type === "p") {
    const dir = forward(color);
    push(file - 1, rank + dir);
    push(file + 1, rank + dir);
    return out;
  }
  if (type === "n") {
    for (const [df, dr] of KNIGHT_DELTAS) push(file + df, rank + dr);
    return out;
  }
  if (type === "k") {
    for (const [df, dr] of KING_DELTAS) push(file + df, rank + dr);
    return out;
  }

  for (const [df, dr] of sliderDirs(type)) {
    let nextFile = file + df;
    let nextRank = rank + dr;
    while (true) {
      const square = squareAt(nextFile, nextRank);
      if (!square) break;
      out.push(square);
      if (pos.at(square)) break;
      nextFile += df;
      nextRank += dr;
    }
  }
  return out;
}

export function attacksSquare(pos: Position, unit: Unit, target: Square): boolean {
  return attackedSquares(pos, unit.square, unit.type, unit.color).includes(target);
}

export function unitsAttacking(pos: Position, target: Square, color: Color): Unit[] {
  return pos.of(color).filter((unit) => attacksSquare(pos, unit, target));
}

export function enemyUnitsAttackedFrom(
  pos: Position,
  square: Square,
  type: PieceSymbol,
  color: Color,
): Unit[] {
  const enemy = opposite(color);
  const hit: Unit[] = [];
  for (const target of attackedSquares(pos, square, type, color)) {
    const unit = pos.at(target);
    if (unit && unit.color === enemy) hit.push(unit);
  }
  return hit;
}

export function enemyUnitsAttackedBy(pos: Position, unit: Unit): Unit[] {
  return enemyUnitsAttackedFrom(pos, unit.square, unit.type, unit.color);
}

/** Pieces met walking from `from` along a direction, nearest first. Empty squares are skipped. */
export function rayUnits(
  pos: Position,
  from: Square,
  df: number,
  dr: number,
): Unit[] {
  const out: Unit[] = [];
  let file = fileIndex(from) + df;
  let rank = rankIndex(from) + dr;
  while (true) {
    const square = squareAt(file, rank);
    if (!square) break;
    const unit = pos.at(square);
    if (unit) out.push(unit);
    file += df;
    rank += dr;
  }
  return out;
}

export function sharesFileOrRank(a: Square, b: Square): boolean {
  return fileIndex(a) === fileIndex(b) || rankIndex(a) === rankIndex(b);
}

export function pieceCountBetween(pos: Position, a: Square, b: Square): number | null {
  const df = fileIndex(b) - fileIndex(a);
  const dr = rankIndex(b) - rankIndex(a);
  if (df === 0 && dr === 0) return null;
  const absFile = Math.abs(df);
  const absRank = Math.abs(dr);
  const aligned = df === 0 || dr === 0 || absFile === absRank;
  if (!aligned) return null;
  const stepFile = Math.sign(df);
  const stepRank = Math.sign(dr);
  let count = 0;
  let file = fileIndex(a) + stepFile;
  let rank = rankIndex(a) + stepRank;
  while (file !== fileIndex(b) || rank !== rankIndex(b)) {
    const square = squareAt(file, rank);
    if (!square) return null;
    if (pos.at(square)) count += 1;
    file += stepFile;
    rank += stepRank;
  }
  return count;
}

export function onSameLine(origin: Square, through: Square, candidate: Square): boolean {
  const between = pieceCountBetween({ units: [], at: () => null, of: () => [], king: () => null }, origin, through);
  if (between === null) return false;
  const toCandidate = pieceCountBetween(
    { units: [], at: () => null, of: () => [], king: () => null },
    origin,
    candidate,
  );
  if (toCandidate === null) return false;
  const df1 = Math.sign(fileIndex(through) - fileIndex(origin));
  const dr1 = Math.sign(rankIndex(through) - rankIndex(origin));
  const df2 = Math.sign(fileIndex(candidate) - fileIndex(origin));
  const dr2 = Math.sign(rankIndex(candidate) - rankIndex(origin));
  return df1 === df2 && dr1 === dr2;
}

export function pawnsOnFile(pos: Position, file: number, color?: Color): Unit[] {
  return pos.units.filter(
    (unit) =>
      unit.type === "p" &&
      fileIndex(unit.square) === file &&
      (color === undefined || unit.color === color),
  );
}

export function clearTravel(pos: Position, unit: Unit): number {
  let max = 0;
  const file = fileIndex(unit.square);
  const rank = rankIndex(unit.square);
  for (const [df, dr] of sliderDirs(unit.type)) {
    let count = 0;
    let nextFile = file + df;
    let nextRank = rank + dr;
    while (true) {
      const square = squareAt(nextFile, nextRank);
      if (!square || pos.at(square)) break;
      count += 1;
      nextFile += df;
      nextRank += dr;
    }
    if (count > max) max = count;
  }
  return max;
}

export function isPassedPawn(pos: Position, pawn: Unit): boolean {
  if (pawn.type !== "p") return false;
  const enemy = opposite(pawn.color);
  const rel = relativeRank(pawn.color, pawn.square);
  const file = fileIndex(pawn.square);
  return !pos.of(enemy, "p").some((other) => {
    const otherFile = fileIndex(other.square);
    if (Math.abs(otherFile - file) > 1) return false;
    return relativeRank(pawn.color, other.square) > rel;
  });
}

export function isIsolatedPawn(pos: Position, pawn: Unit): boolean {
  if (pawn.type !== "p") return false;
  const file = fileIndex(pawn.square);
  return !pos.of(pawn.color, "p").some((other) => Math.abs(fileIndex(other.square) - file) === 1);
}

export function enPassantVictimSquare(from: Square, to: Square): Square {
  return `${to[0]}${from[1]}` as Square;
}
