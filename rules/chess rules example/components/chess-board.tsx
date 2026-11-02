import { Chess, type Square } from "chess.js";
import { pieceName, type Color, type PieceSymbol } from "@/lib/chess/geometry";

const FILES = ["a", "b", "c", "d", "e", "f", "g", "h"] as const;
const RANKS = [8, 7, 6, 5, 4, 3, 2, 1] as const;
const LETTER: Record<PieceSymbol, string> = {
  k: "K",
  q: "Q",
  r: "R",
  b: "B",
  n: "N",
  p: "P",
};

export function ChessBoard({
  fen,
  participants,
  targets,
}: {
  fen: string;
  participants: Square[];
  targets: Square[];
}) {
  const chess = new Chess(fen);
  const participantSet = new Set(participants);
  const targetSet = new Set(targets);

  return (
    <div className="w-full max-w-[26rem]">
      <div className="grid grid-cols-[1.25rem_1fr] gap-1">
        <div className="grid grid-rows-8">
          {RANKS.map((rank) => (
            <div key={rank} className="grid place-items-center text-xs text-muted-foreground">
              {rank}
            </div>
          ))}
        </div>
        <div
          role="grid"
          aria-label="Chessboard"
          className="grid aspect-square grid-cols-8 overflow-hidden rounded-lg ring-1 ring-foreground/15"
        >
          {RANKS.flatMap((rank) =>
            FILES.map((file, fileIndex) => {
              const square = `${file}${rank}` as Square;
              const piece = chess.get(square);
              const dark = (fileIndex + rank) % 2 === 0;
              const participating = participantSet.has(square);
              const targeted = targetSet.has(square);
              const label = [
                square,
                piece ? `${piece.color === "w" ? "white" : "black"} ${pieceName(piece.type)}` : "empty",
                participating ? "participating piece" : "",
                targeted ? "capture target" : "",
              ]
                .filter(Boolean)
                .join(", ");
              return (
                <div
                  key={square}
                  role="gridcell"
                  aria-label={label}
                  className={`relative grid place-items-center ${dark ? "bg-[oklch(0.46_0.055_155)]" : "bg-[oklch(0.9_0.02_90)]"} ${
                    participating ? "outline outline-2 -outline-offset-4 outline-[oklch(0.78_0.12_155)]" : ""
                  }`}
                >
                  {targeted ? (
                    <span className="absolute top-1 right-1 size-1.5 rounded-full bg-[oklch(0.55_0.16_30)]" />
                  ) : null}
                  {piece ? <PieceToken type={piece.type} color={piece.color} /> : null}
                </div>
              );
            }),
          )}
        </div>
      </div>
      <div className="mt-1 grid grid-cols-[1.25rem_1fr] gap-1">
        <span />
        <div className="grid grid-cols-8">
          {FILES.map((file) => (
            <div key={file} className="text-center text-xs text-muted-foreground">
              {file}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

function PieceToken({ type, color }: { type: PieceSymbol; color: Color }) {
  return (
    <span
      className={`grid size-[74%] place-items-center rounded-full font-heading text-[clamp(0.8rem,2.4vw,1.15rem)] leading-none font-semibold ${
        color === "w"
          ? "bg-[oklch(0.96_0.01_90)] text-[oklch(0.22_0.02_55)] ring-1 ring-black/25"
          : "bg-[oklch(0.22_0.02_55)] text-[oklch(0.96_0.01_90)]"
      }`}
    >
      {LETTER[type]}
    </span>
  );
}
