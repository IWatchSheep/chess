import { Chess, type Move } from "chess.js";
import {
  KING_COMPARE,
  MATE_BONUS,
  enPassantVictimSquare,
  materialValue,
  type Color,
  type Square,
} from "./geometry";
import { cloneChess } from "./position";

export interface CaptureValuation {
  /** Total value of opposing pieces taken by the original player in the priced sequence. */
  expectedGain: number;
  /** Total value of the original player's pieces taken by the opponent in that sequence. */
  potentialLoss: number;
  net: number;
  mate: boolean;
  /** Recaptures that are actually played after the initial capture. */
  recaptureSans: string[];
}

interface Outcome {
  expectedGain: number;
  potentialLoss: number;
  stopped: boolean;
  sequence: string[];
}

const STOP: Outcome = {
  expectedGain: 0,
  potentialLoss: 0,
  stopped: true,
  sequence: [],
};

function netOf(outcome: Outcome): number {
  return outcome.expectedGain - outcome.potentialLoss;
}

function prefer(maximizeOriginal: boolean, options: Outcome[]): Outcome {
  let best = options[0];
  for (let index = 1; index < options.length; index += 1) {
    const candidate = options[index];
    const delta = netOf(candidate) - netOf(best);
    if (maximizeOriginal ? delta > 0 : delta < 0) {
      best = candidate;
    } else if (delta === 0 && candidate.stopped && !best.stopped) {
      best = candidate;
    }
  }
  return best;
}

function promotionExtra(move: Move): number {
  if (!move.promotion || move.promotion === "k") return 0;
  return materialValue(move.promotion) - materialValue("p");
}

function capturedMaterial(chess: Chess, move: Move): number {
  if (move.isEnPassant()) return materialValue("p");
  const victim = chess.get(move.to);
  if (!victim || victim.type === "k") return 0;
  return materialValue(victim.type);
}

function standingValue(move: Move): number {
  if (move.piece === "k") return KING_COMPARE;
  return materialValue(move.piece);
}

const PROMOTION_RANK: Record<string, number> = { q: 4, r: 3, b: 2, n: 1 };

function recapturesOn(chess: Chess, square: Square): Move[] {
  return chess
    .moves({ verbose: true })
    .filter((move) => move.isCapture() && move.to === square)
    .sort((a, b) => {
      if (a.from !== b.from) return a.from < b.from ? -1 : 1;
      return (PROMOTION_RANK[b.promotion ?? ""] ?? 0) - (PROMOTION_RANK[a.promotion ?? ""] ?? 0);
    });
}

/**
 * Optimal static exchange on one square after a capture has already been played.
 * Only the cheapest standing piece value may recapture. Every attacker of that
 * value is tried. Each side then stops when continuing does not improve its result.
 * Equal nets stop rather than trade more material.
 */
function exchange(chess: Chess, square: Square, original: Color): Outcome {
  if (chess.isGameOver()) return STOP;
  const side = chess.turn();
  const moves = recapturesOn(chess, square);
  if (moves.length === 0) return STOP;

  const cheapest = Math.min(...moves.map(standingValue));
  const candidates = moves.filter((move) => standingValue(move) === cheapest);
  const maximize = side === original;
  const considered: Outcome[] = [STOP];

  for (const move of candidates) {
    const taken = capturedMaterial(chess, move) + promotionExtra(move);
    const next = cloneChess(chess);
    const played = next.move({
      from: move.from,
      to: move.to,
      promotion: move.promotion,
    });

    if (next.isCheckmate()) {
      considered.push(
        side === original
          ? {
              expectedGain: taken + MATE_BONUS,
              potentialLoss: 0,
              stopped: false,
              sequence: [played.san],
            }
          : {
              expectedGain: 0,
              potentialLoss: taken + MATE_BONUS,
              stopped: false,
              sequence: [played.san],
            },
      );
      continue;
    }

    const continuation = exchange(next, square, original);
    considered.push(
      side === original
        ? {
            expectedGain: taken + continuation.expectedGain,
            potentialLoss: continuation.potentialLoss,
            stopped: false,
            sequence: [played.san, ...continuation.sequence],
          }
        : {
            expectedGain: continuation.expectedGain,
            potentialLoss: taken + continuation.potentialLoss,
            stopped: false,
            sequence: [played.san, ...continuation.sequence],
          },
    );
  }

  return prefer(maximize, considered);
}

/**
 * Price one legal capture. The capture itself is always committed. Recaptures
 * after it use optimal stopping. Promotion adds the gained piece value minus a pawn.
 */
export function valueCapture(chess: Chess, move: Move): CaptureValuation {
  const original = move.color;
  const initial = capturedMaterial(chess, move) + promotionExtra(move);
  const next = cloneChess(chess);
  next.move({ from: move.from, to: move.to, promotion: move.promotion });

  if (next.isCheckmate()) {
    const expectedGain = initial + MATE_BONUS;
    return {
      expectedGain,
      potentialLoss: 0,
      net: expectedGain,
      mate: true,
      recaptureSans: [],
    };
  }

  const continuation = exchange(next, move.to, original);
  const expectedGain = initial + continuation.expectedGain;
  const potentialLoss = continuation.potentialLoss;
  return {
    expectedGain,
    potentialLoss,
    net: expectedGain - potentialLoss,
    mate: expectedGain >= MATE_BONUS || potentialLoss >= MATE_BONUS,
    recaptureSans: continuation.sequence,
  };
}

export function victimSquareOf(move: Move): Square {
  if (move.isEnPassant()) return enPassantVictimSquare(move.from, move.to);
  return move.to;
}
