import { Chess, type Move } from "chess.js";
import { MATE_BONUS, pieceName, sideName, type Color, type PieceSymbol, type Square, type Unit } from "./geometry";
import { victimSquareOf, valueCapture, type CaptureValuation } from "./exchange";
import {
  cloneChess,
  makeContext,
  playSans,
  recordMove,
  type EvalContext,
  type RecordedMove,
} from "./position";
import { STRATEGIES, type StrategyDefinition, type StrategyReading } from "./strategies";

export type Stage = "beginning" | "partial" | "full";

export const STAGE_LABEL: Record<Stage, string> = {
  beginning: "Beginning to form",
  partial: "Partially developed",
  full: "Fully developed",
};

export interface ScoredMilestones {
  stage: Stage;
  score: number;
  progressWithinTurn: boolean;
  completionWithinTurn: boolean;
}

/**
 * Development score for one strategy on one position.
 *
 * Completion now is fully developed, score 100.
 * Foundation and progress now score 60, plus 15 when completion appears within one full turn.
 * Foundation alone, or progress alone, is beginning at 30.
 * Foundation now plus progress within one full turn is beginning at 45.
 * A future line never creates a strategy by itself.
 */
export function scoreMilestones(
  now: Pick<StrategyReading, "foundation" | "progress" | "completion">,
  future: { progress: boolean; completion: boolean },
): ScoredMilestones | null {
  if (now.completion) {
    return {
      stage: "full",
      score: 100,
      progressWithinTurn: false,
      completionWithinTurn: false,
    };
  }
  if (!now.foundation && !now.progress) return null;

  const progressWithinTurn = Boolean(now.foundation && !now.progress && (future.progress || future.completion));
  const completionWithinTurn = Boolean(now.foundation && now.progress && future.completion);
  let score = 0;
  if (now.foundation) score += 30;
  if (now.progress) score += 30;
  else if (progressWithinTurn) score += 15;
  if (completionWithinTurn) score += 15;

  return {
    stage: now.foundation && now.progress ? "partial" : "beginning",
    score,
    progressWithinTurn,
    completionWithinTurn,
  };
}

export function explainScore(scored: ScoredMilestones, now: StrategyReading): string {
  if (now.completion) {
    return "Completion is true on this position, so the strategy is fully developed.";
  }
  if (scored.completionWithinTurn) {
    return "Foundation and progress are already on the board, and a legal line within one full turn reaches completion. That is partial, score 75, until the completing move is the position itself.";
  }
  if (now.foundation && now.progress) {
    return "Foundation and progress are on the board. Completion is not, and it does not appear within one full turn.";
  }
  if (scored.progressWithinTurn) {
    return "The foundation is on the board. The progress condition is not met yet, but a legal line within one full turn reaches it.";
  }
  if (now.foundation) {
    return "Only the foundation is present. The progress move is not available within one full turn.";
  }
  return "Progress is visible without the foundation. The plan has started, and the structural base is not on the board.";
}

export interface CaptureRow {
  strategyId: string;
  strategyName: string;
  san: string;
  from: Square;
  to: Square;
  capturerType: PieceSymbol;
  victimType: PieceSymbol;
  victimSquare: Square;
  expectedGain: number;
  potentialLoss: number;
  net: number;
  mate: boolean;
  recaptureSans: string[];
}

export interface ValuedCapture {
  san: string;
  from: Square;
  to: Square;
  capturerType: PieceSymbol;
  victimType: PieceSymbol;
  victimSquare: Square;
  valuation: CaptureValuation;
  strategies: { id: string; name: string }[];
}

export interface StrategyHit {
  id: string;
  name: string;
  intent: string;
  stage: Stage;
  stageLabel: string;
  score: number;
  foundation: boolean;
  progress: boolean;
  completion: boolean;
  progressWithinTurn: boolean;
  completionWithinTurn: boolean;
  explanation: string;
  evidence: string;
  milestones: {
    foundation: string;
    progress: string;
    completion: string;
  };
  captureLink: string;
  participants: Unit[];
  targets: Unit[];
  captures: CaptureRow[];
}

export interface Evaluation {
  side: Color;
  turn: Color;
  fen: string;
  sideToMove: boolean;
  hits: StrategyHit[];
  captures: CaptureRow[];
  legalCaptures: ValuedCapture[];
}

function sortHits(hits: StrategyHit[]): StrategyHit[] {
  const rank: Record<Stage, number> = { full: 0, partial: 1, beginning: 2 };
  return [...hits].sort(
    (a, b) => rank[a.stage] - rank[b.stage] || b.score - a.score || a.name.localeCompare(b.name),
  );
}

function sortCaptures(rows: CaptureRow[]): CaptureRow[] {
  return [...rows].sort(
    (a, b) =>
      b.net - a.net ||
      b.expectedGain - a.expectedGain ||
      a.potentialLoss - b.potentialLoss ||
      a.strategyName.localeCompare(b.strategyName) ||
      a.san.localeCompare(b.san) ||
      a.victimSquare.localeCompare(b.victimSquare),
  );
}

function associateCaptures(
  ctx: EvalContext,
  strategy: StrategyDefinition,
  reading: StrategyReading,
): CaptureRow[] {
  if (ctx.chess.turn() !== ctx.side) return [];
  const participants = new Set(
    reading.participants.filter((unit) => unit.color === ctx.side).map((unit) => unit.square),
  );
  const targets = new Set(reading.targets.map((unit) => unit.square));
  const rows: CaptureRow[] = [];
  for (const move of ctx.chess.moves({ verbose: true })) {
    if (!move.isCapture() || !participants.has(move.from)) continue;
    const victimSquare = victimSquareOf(move);
    if (!targets.has(victimSquare) || !move.captured) continue;
    const valuation = valueCapture(ctx.chess, move);
    rows.push({
      strategyId: strategy.id,
      strategyName: strategy.name,
      san: move.san,
      from: move.from,
      to: move.to,
      capturerType: move.piece,
      victimType: move.captured,
      victimSquare,
      expectedGain: valuation.expectedGain,
      potentialLoss: valuation.potentialLoss,
      net: valuation.net,
      mate: valuation.mate,
      recaptureSans: valuation.recaptureSans,
    });
  }
  return rows;
}

function needsFuture(reading: StrategyReading, future: { progress: boolean; completion: boolean }): boolean {
  if (reading.completion || !reading.foundation) return false;
  if (reading.progress) return !future.completion;
  return !future.progress;
}

function searchOneFullTurn(
  ctx: EvalContext,
  now: Map<string, StrategyReading>,
  future: Map<string, { progress: boolean; completion: boolean }>,
) {
  const side = ctx.side;

  const unfinished = () => STRATEGIES.some((strategy) => needsFuture(now.get(strategy.id)!, future.get(strategy.id)!));

  const absorb = (chess: Chess, moves: RecordedMove[]) => {
    const child = makeContext(chess, side, moves);
    for (const strategy of STRATEGIES) {
      const current = now.get(strategy.id)!;
      const slot = future.get(strategy.id)!;
      if (!needsFuture(current, slot)) continue;
      const reading = strategy.read(child);
      if (reading.completion) {
        slot.completion = true;
        slot.progress = true;
      } else if (reading.progress) {
        slot.progress = true;
      }
    }
  };

  const walk = (chess: Chess, moves: RecordedMove[], pliesLeft: number) => {
    if (pliesLeft === 0 || !unfinished() || chess.isGameOver()) return;
    for (const move of chess.moves({ verbose: true })) {
      if (!unfinished()) return;
      const next = cloneChess(chess);
      const played = next.move({ from: move.from, to: move.to, promotion: move.promotion });
      const nextMoves = [...moves, recordMove(played)];
      absorb(next, nextMoves);
      if (!next.isGameOver()) walk(next, nextMoves, pliesLeft - 1);
    }
  };

  walk(ctx.chess, ctx.moves, 2);
}

export function evaluatePosition(chess: Chess, side: Color, moves: RecordedMove[] = []): Evaluation {
  const ctx = makeContext(chess, side, moves);
  const now = new Map<string, StrategyReading>(STRATEGIES.map((strategy) => [strategy.id, strategy.read(ctx)]));
  const future = new Map<string, { progress: boolean; completion: boolean }>(
    STRATEGIES.map((strategy) => [strategy.id, { progress: false, completion: false }]),
  );
  if (STRATEGIES.some((strategy) => needsFuture(now.get(strategy.id)!, future.get(strategy.id)!))) {
    searchOneFullTurn(ctx, now, future);
  }

  const hits: StrategyHit[] = [];
  for (const strategy of STRATEGIES) {
    const reading = now.get(strategy.id)!;
    const ahead = future.get(strategy.id)!;
    const scored = scoreMilestones(reading, ahead);
    if (!scored) continue;
    const captures = associateCaptures(ctx, strategy, reading);
    hits.push({
      id: strategy.id,
      name: strategy.name,
      intent: strategy.intent,
      stage: scored.stage,
      stageLabel: STAGE_LABEL[scored.stage],
      score: scored.score,
      foundation: reading.foundation,
      progress: reading.progress,
      completion: reading.completion,
      progressWithinTurn: scored.progressWithinTurn,
      completionWithinTurn: scored.completionWithinTurn,
      explanation: explainScore(scored, reading),
      evidence: reading.evidence,
      milestones: {
        foundation: strategy.foundation,
        progress: strategy.progress,
        completion: strategy.completion,
      },
      captureLink: strategy.captureLink,
      participants: reading.participants,
      targets: reading.targets,
      captures,
    });
  }

  const ordered = sortHits(hits);
  const captures = sortCaptures(ordered.flatMap((hit) => hit.captures));
  const legalCaptures = valueLegalCaptures(chess, side, ordered);

  return {
    side,
    turn: chess.turn(),
    fen: chess.fen(),
    sideToMove: chess.turn() === side,
    hits: ordered,
    captures,
    legalCaptures,
  };
}

function valueLegalCaptures(chess: Chess, side: Color, hits: StrategyHit[]): ValuedCapture[] {
  if (chess.turn() !== side) return [];
  const rows: ValuedCapture[] = [];
  for (const move of chess.moves({ verbose: true })) {
    if (!move.isCapture() || !move.captured) continue;
    const victimSquare = victimSquareOf(move);
    const strategies = hits
      .filter((hit) =>
        hit.captures.some((capture) => capture.from === move.from && capture.to === move.to && capture.san === move.san),
      )
      .map((hit) => ({ id: hit.id, name: hit.name }));
    rows.push({
      san: move.san,
      from: move.from,
      to: move.to,
      capturerType: move.piece,
      victimType: move.captured,
      victimSquare,
      valuation: valueCapture(chess, move),
      strategies,
    });
  }
  rows.sort(
    (a, b) =>
      b.valuation.net - a.valuation.net ||
      b.valuation.expectedGain - a.valuation.expectedGain ||
      a.valuation.potentialLoss - b.valuation.potentialLoss ||
      a.san.localeCompare(b.san),
  );
  return rows;
}

export function evaluateFen(fen: string, side: Color, moves: RecordedMove[] = []): Evaluation {
  return evaluatePosition(new Chess(fen), side, moves);
}

export function evaluateLine(sans: string[], side?: Color): Evaluation {
  const { chess, moves } = playSans(sans);
  return evaluatePosition(chess, side ?? chess.turn(), moves);
}

export function legalMoveList(fen: string): Move[] {
  return new Chess(fen).moves({ verbose: true });
}

export function describeCapture(row: Pick<CaptureRow, "expectedGain" | "potentialLoss" | "net" | "mate" | "recaptureSans" | "san">): string {
  const reply = row.recaptureSans.length > 0 ? ` Reply: ${row.recaptureSans.join(" ")}.` : " No recapture follows.";
  const mate = row.mate ? " The sequence includes checkmate." : "";
  return `${row.san} gains ${formatMaterial(row.expectedGain)} and risks ${formatMaterial(row.potentialLoss)}. Net ${formatSigned(row.net)}.${reply}${mate}`;
}

export function formatMaterial(value: number): string {
  if (value >= MATE_BONUS) {
    const rest = value - MATE_BONUS;
    return rest > 0 ? `mate + ${rest}` : "mate";
  }
  return String(value);
}

export function formatSigned(value: number): string {
  if (value >= MATE_BONUS) return formatMaterial(value);
  if (value <= -MATE_BONUS) return `−${formatMaterial(-value)}`;
  return value > 0 ? `+${value}` : String(value);
}

export function captureHeading(type: PieceSymbol, square: Square): string {
  return `${pieceName(type)} on ${square}`;
}

export function evaluationTitle(side: Color): string {
  return `${sideName(side)} strategies`;
}
