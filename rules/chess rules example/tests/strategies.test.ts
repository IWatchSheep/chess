import { describe, expect, it } from "vitest";
import { evaluateFen, evaluateLine, PRESETS, presetById, type StrategyHit } from "../lib/chess";

function hit(hits: StrategyHit[], id: string) {
  return hits.find((item) => item.id === id);
}

function expectStage(hits: StrategyHit[], id: string, stage: StrategyHit["stage"], score: number) {
  const found = hit(hits, id);
  expect(found, id).toBeTruthy();
  expect(found).toMatchObject({ stage, score });
}

describe("development scoring", () => {
  it("marks the starting position as two beginning strategies at 45", () => {
    const evaluation = evaluateFen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", "w");
    expect(evaluation.hits.map((item) => `${item.id}:${item.score}:${item.stage}`).sort()).toEqual([
      "central-pawn-occupation:45:beginning",
      "king-shelter:45:beginning",
    ]);
    expect(evaluation.hits.every((item) => item.progressWithinTurn && !item.completion)).toBe(true);
  });

  it("raises central occupation to partial once the e-pawn arrives", () => {
    const evaluation = evaluateLine(["e4"], "w");
    expectStage(evaluation.hits, "central-pawn-occupation", "partial", 60);
    expect(evaluation.sideToMove).toBe(false);
    expect(evaluation.captures).toEqual([]);
  });

  it("scores a completable center at 75 rather than fully developed", () => {
    const evaluation = evaluateFen("4k3/8/8/8/4P3/2P2P2/3P4/4K3 w - - 0 1", "w");
    expectStage(evaluation.hits, "central-pawn-occupation", "partial", 75);
    expect(hit(evaluation.hits, "central-pawn-occupation")?.completionWithinTurn).toBe(true);
    expect(hit(evaluation.hits, "central-pawn-occupation")?.completion).toBe(false);
  });

  it("does not list a strategy from a future line alone", () => {
    const evaluation = evaluateFen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", "w");
    expect(hit(evaluation.hits, "open-file-control")).toBeUndefined();
    expect(hit(evaluation.hits, "kingside-pawn-storm")).toBeUndefined();
  });
});

describe("the twenty strategies", () => {
  it("recognizes every strategy as fully developed in its reference position", () => {
    const cases: { preset: string; id: string }[] = [
      { preset: "supported-center", id: "central-pawn-occupation" },
      { preset: "central-break", id: "central-pawn-break" },
      { preset: "storm", id: "kingside-pawn-storm" },
      { preset: "minority", id: "queenside-minority-attack" },
      { preset: "king-shelter", id: "king-shelter" },
      { preset: "opposite", id: "opposite-side-castling-attack" },
      { preset: "fianchetto", id: "fianchetto" },
      { preset: "battery", id: "open-file-control" },
      { preset: "seventh", id: "seventh-rank-invasion" },
      { preset: "passed-pawn", id: "passed-pawn-advance" },
      { preset: "isolated", id: "isolated-pawn-attack" },
      { preset: "backward", id: "backward-pawn-pressure" },
      { preset: "outpost", id: "knight-outpost" },
      { preset: "bishop-pair", id: "bishop-pair" },
      { preset: "pin", id: "pin" },
      { preset: "discovery", id: "discovered-attack" },
      { preset: "fork", id: "fork" },
      { preset: "skewer", id: "skewer" },
      { preset: "battery", id: "major-piece-battery" },
      { preset: "promotion", id: "promotion" },
    ];

    const failures: string[] = [];
    for (const testCase of cases) {
      const preset = presetById(testCase.preset);
      const evaluation = evaluateFen(preset.fen, preset.side, preset.moves);
      const found = hit(evaluation.hits, testCase.id);
      if (!found || found.stage !== "full" || found.score !== 100) {
        const summary = evaluation.hits.map((item) => `${item.id}:${item.stage}:${item.score}`).join(", ");
        failures.push(`${testCase.id} via ${testCase.preset} -> ${found ? `${found.stage}:${found.score} (${found.evidence})` : "missing"}; saw ${summary}`);
      }
    }
    expect(failures).toEqual([]);
  });

  it("sorts fully developed strategies above partial and beginning ones", () => {
    const preset = presetById("supported-center");
    const evaluation = evaluateFen(preset.fen, "w");
    const stages = evaluation.hits.map((item) => item.stage);
    const rank = { full: 0, partial: 1, beginning: 2 };
    const sorted = [...stages].sort((a, b) => rank[a] - rank[b]);
    expect(stages).toEqual(sorted);
    expect(stages[0]).toBe("full");
  });

  it("keeps a blocked promotion partial when the pawn cannot legally promote", () => {
    const evaluation = evaluateFen("4k3/4P3/8/8/8/8/8/4K3 w - - 0 1", "w");
    const found = hit(evaluation.hits, "promotion");
    expect(found?.completion).toBe(false);
    expect(found?.stage).not.toBe("full");
  });

  it("still counts a pinned bishop toward a pin when the bishop itself has the line", () => {
    const evaluation = evaluateFen("4k3/8/2n5/1B6/8/8/8/4K3 w - - 0 1", "w");
    const found = hit(evaluation.hits, "pin");
    expect(found?.participants.some((unit) => unit.square === "b5")).toBe(true);
    expect(found?.targets.some((unit) => unit.square === "c6")).toBe(true);
  });
});

describe("strategy captures", () => {
  it("links the bishop's capture of the pinned knight and prices the recapture", () => {
    const evaluation = evaluateFen("4k3/8/2n5/1B6/8/8/8/4K3 w - - 0 1", "w");
    const capture = evaluation.captures.find((row) => row.san.startsWith("Bxc6") && row.strategyId === "pin");
    expect(capture).toBeTruthy();
    expect(capture).toMatchObject({ expectedGain: 320, potentialLoss: 0, net: 320 });
  });

  it("sorts strategy captures by net value", () => {
    const preset = presetById("fork");
    const evaluation = evaluateFen(preset.fen, "w");
    const nets = evaluation.captures.map((row) => row.net);
    expect(nets).toEqual([...nets].sort((a, b) => b - a));
    expect(evaluation.captures.some((row) => row.strategyId === "fork" && row.victimType === "q")).toBe(true);
  });

  it("prices every legal capture, including ones no strategy claims", () => {
    const evaluation = evaluateFen("4k3/8/8/3p4/8/8/8/3QK3 w - - 0 1", "w");
    const capture = evaluation.legalCaptures.find((row) => row.san === "Qxd5");
    expect(capture?.valuation.net).toBe(100);
  });

  it("loads every preset without throwing", () => {
    for (const preset of PRESETS) {
      const evaluation = evaluateFen(preset.fen, preset.side, preset.moves);
      expect(evaluation.fen).toContain(preset.side === "w" || preset.side === "b" ? " " : " ");
      expect(evaluation.hits.length).toBeGreaterThan(0);
    }
  });
});
