import { Chess } from "chess.js";
import { describe, expect, it } from "vitest";
import { MATE_BONUS, valueCapture } from "../lib/chess";

function price(fen: string, san: string) {
  const chess = new Chess(fen);
  const move = chess.moves({ verbose: true }).find((candidate) => candidate.san === san);
  expect(move, `missing ${san} in ${fen}\n${chess.ascii()}`).toBeTruthy();
  return valueCapture(chess, move!);
}

describe("capture valuation", () => {
  it("prices an undefended pawn as pure gain", () => {
    const result = price("4k3/8/8/3p4/8/8/8/3QK3 w - - 0 1", "Qxd5");
    expect(result).toMatchObject({ expectedGain: 100, potentialLoss: 0, net: 100, mate: false, recaptureSans: [] });
  });

  it("charges the queen when a knight recaptures it", () => {
    const result = price("4k3/4n3/8/3p4/8/8/8/3QK3 w - - 0 1", "Qxd5");
    expect(result.expectedGain).toBe(100);
    expect(result.potentialLoss).toBe(900);
    expect(result.net).toBe(-800);
    expect(result.recaptureSans).toEqual(["Nxd5"]);
  });

  it("stops the opponent from recapturing into a lost rook", () => {
    const result = price("4r2k/8/8/4q3/3P4/8/8/4R1K1 w - - 0 1", "dxe5");
    expect(result).toMatchObject({ expectedGain: 900, potentialLoss: 0, net: 900, recaptureSans: [] });
  });

  it("continues when the knight sacrifice wins the queen and the pawn", () => {
    const result = price("4k3/4p3/5q2/6P1/6N1/8/8/4K3 w - - 0 1", "Nxf6+");
    expect(result.expectedGain).toBe(1000);
    expect(result.potentialLoss).toBe(320);
    expect(result.net).toBe(680);
    expect(result.recaptureSans).toEqual(["exf6", "gxf6"]);
  });

  it("recaptures with the pawn rather than the queen", () => {
    const result = price("3q3k/8/2p5/3r4/8/8/8/3QK3 w - - 0 1", "Qxd5");
    expect(result.expectedGain).toBe(500);
    expect(result.potentialLoss).toBe(900);
    expect(result.net).toBe(-400);
    expect(result.recaptureSans[0]).toBe("cxd5");
  });

  it("prices en passant and the rook recapture", () => {
    const result = price("3rk3/8/8/3pP3/8/8/8/4K3 w - d6 0 1", "exd6");
    expect(result).toMatchObject({ expectedGain: 100, potentialLoss: 100, net: 0 });
    expect(result.recaptureSans).toEqual(["Rxd6"]);
  });

  it("adds the mate bonus and no loss when the capture checkmates", () => {
    const result = price("k7/1p6/1Q6/2N5/8/8/8/4K3 w - - 0 1", "Qxb7#");
    expect(result.mate).toBe(true);
    expect(result.potentialLoss).toBe(0);
    expect(result.expectedGain).toBe(100 + MATE_BONUS);
    expect(result.net).toBe(100 + MATE_BONUS);
  });

  it("adds promotion value before a recapture of the new queen", () => {
    const chess = new Chess("7r/5P1k/8/8/8/8/8/4K3 w - - 0 1");
    const move = chess.moves({ verbose: true }).find((candidate) => candidate.san === "f8=Q");
    expect(move).toBeTruthy();
    const result = valueCapture(chess, move!);
    expect(result.expectedGain).toBe(800);
    expect(result.potentialLoss).toBe(900);
    expect(result.net).toBe(-100);
    expect(result.recaptureSans).toEqual(["Rxf8"]);
  });
});
