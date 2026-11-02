import { Chess, type Move } from "chess.js";
import {
  type Color,
  type PieceSymbol,
  type Position,
  type Square,
  type Unit,
} from "./geometry";

export function readPosition(chess: Chess): Position {
  const units: Unit[] = [];
  for (const row of chess.board()) {
    for (const cell of row) {
      if (!cell) continue;
      units.push({ square: cell.square, type: cell.type, color: cell.color });
    }
  }
  const bySquare = new Map<Square, Unit>(units.map((unit) => [unit.square, unit]));
  return {
    units,
    at(square) {
      return bySquare.get(square) ?? null;
    },
    of(color, type) {
      return units.filter((unit) => unit.color === color && (type === undefined || unit.type === type));
    },
    king(color) {
      return units.find((unit) => unit.color === color && unit.type === "k") ?? null;
    },
  };
}

export interface RecordedMove {
  color: Color;
  from: Square;
  to: Square;
  piece: PieceSymbol;
  captured?: PieceSymbol;
  promotion?: PieceSymbol;
  san: string;
  enPassant: boolean;
}

export function recordMove(move: Move): RecordedMove {
  return {
    color: move.color,
    from: move.from,
    to: move.to,
    piece: move.piece,
    captured: move.captured,
    promotion: move.promotion,
    san: move.san,
    enPassant: move.isEnPassant(),
  };
}

/**
 * Previous four turns by `side`, plus the current turn when that turn has already
 * been played. When it is `side`'s turn, the current turn has not been played yet.
 */
export function historyWindow(moves: RecordedMove[], side: Color, turn: Color): RecordedMove[] {
  const own = moves.filter((move) => move.color === side);
  const count = turn === side ? 4 : 5;
  return own.slice(-count);
}

export interface EvalContext {
  chess: Chess;
  pos: Position;
  side: Color;
  moves: RecordedMove[];
  window: RecordedMove[];
}

export function makeContext(chess: Chess, side: Color, moves: RecordedMove[]): EvalContext {
  return {
    chess,
    pos: readPosition(chess),
    side,
    moves,
    window: historyWindow(moves, side, chess.turn()),
  };
}

export function playSans(sans: string[]): { chess: Chess; moves: RecordedMove[] } {
  const chess = new Chess();
  const moves = sans.map((san) => recordMove(chess.move(san)));
  return { chess, moves };
}

/**
 * Legal moves for `side`. When it is not that side's turn, the turn is flipped
 * so recognition can see moves the side would have if the opponent passed.
 * Capture valuation does not use this. It only prices a capture on the true turn.
 */
export function movesFor(chess: Chess, side: Color): Move[] {
  if (chess.turn() === side) return chess.moves({ verbose: true });
  const copy = new Chess(chess.fen());
  const flipped = copy.setTurn(side);
  if (!flipped) return [];
  return copy.moves({ verbose: true });
}

export function cloneChess(chess: Chess): Chess {
  return new Chess(chess.fen());
}
