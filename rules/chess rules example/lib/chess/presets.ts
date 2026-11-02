import type { Color } from "./geometry";
import { playSans, type RecordedMove } from "./position";

export interface Preset {
  id: string;
  group: "Opening" | "Structures" | "Tactics" | "Captures";
  name: string;
  detail: string;
  fen: string;
  moves: RecordedMove[];
  side: Color;
}

function fromSans(
  id: string,
  group: Preset["group"],
  name: string,
  detail: string,
  sans: string[],
  side?: Color,
): Preset {
  const played = playSans(sans);
  return {
    id,
    group,
    name,
    detail,
    fen: played.chess.fen(),
    moves: played.moves,
    side: side ?? played.chess.turn(),
  };
}

function fromFen(
  id: string,
  group: Preset["group"],
  name: string,
  detail: string,
  fen: string,
  side: Color = "w",
): Preset {
  return { id, group, name, detail, fen, moves: [], side };
}

export const PRESETS: Preset[] = [
  fromFen(
    "start",
    "Opening",
    "Starting position",
    "Central occupation and king shelter are only foundations. Both can take their next step within one full turn.",
    "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
  ),
  fromSans(
    "after-e4",
    "Opening",
    "After 1.e4",
    "White's e-pawn has reached the central square. Occupation is partial. Black is to move, so White's capture list waits.",
    ["e4"],
    "w",
  ),
  fromSans(
    "central-break",
    "Opening",
    "Central break played",
    "1.e4 d5 2.exd5. The pawn capture sits in White's turn window, so the break is fully developed.",
    ["e4", "d5", "exd5"],
    "w",
  ),
  fromFen(
    "supported-center",
    "Structures",
    "Supported pawn center",
    "Pawns on d4 and e4, defended by c3 and f3.",
    "4k3/8/8/8/3PP3/2P2P2/8/4K3 w - - 0 1",
  ),
  fromFen(
    "king-shelter",
    "Structures",
    "Kingside shelter",
    "King on g1 behind unmoved f-, g-, and h-pawns.",
    "4k3/8/8/8/8/8/5PPP/6K1 w - - 0 1",
  ),
  fromFen(
    "fianchetto",
    "Structures",
    "Kingside fianchetto",
    "Bishop on g2, pawn on g3, diagonal open toward d5 and e4.",
    "4k3/8/8/8/8/6P1/6B1/4K3 w - - 0 1",
  ),
  fromFen(
    "minority",
    "Structures",
    "Queenside minority",
    "White has one queenside pawn against three, advanced to b5. Black's c-pawns are doubled.",
    "4k3/p1p5/2p5/1P6/8/8/8/4K3 w - - 0 1",
  ),
  fromFen(
    "passed-pawn",
    "Structures",
    "Defended passed pawn",
    "The e6-pawn is passed and defended by the pawn on f5.",
    "4k3/8/4P3/5P2/8/8/8/4K3 w - - 0 1",
  ),
  fromFen(
    "promotion",
    "Structures",
    "Pawn ready to promote",
    "The e7-pawn has a legal move to e8.",
    "7k/4P3/8/8/8/8/8/4K3 w - - 0 1",
  ),
  fromFen(
    "outpost",
    "Structures",
    "Knight outpost",
    "Knight on d5, defended by e4, with no black pawn able to attack the square.",
    "4k3/8/8/3N4/4P3/8/8/4K3 w - - 0 1",
  ),
  fromFen(
    "bishop-pair",
    "Structures",
    "Bishop pair",
    "Two bishops against none, with the d-file open and the dark bishop developed.",
    "4k3/8/8/8/3B4/8/8/4KB2 w - - 0 1",
  ),
  fromFen(
    "isolated",
    "Structures",
    "Isolated pawn",
    "Black's e5-pawn has no neighbor. The bishop and rook attack it.",
    "4k3/p7/8/4p3/8/2B5/8/4R1K1 w - - 0 1",
  ),
  fromFen(
    "backward",
    "Structures",
    "Backward pawn",
    "Black's d6-pawn advances to d5, which the e4-pawn attacks. No black pawn defends d6, and the rook is on the d-file.",
    "4k3/8/3p4/8/4P3/8/8/3RK3 w - - 0 1",
  ),
  fromFen(
    "battery",
    "Structures",
    "Doubled rooks",
    "Rooks on e1 and e2 own the open e-file.",
    "4k3/8/8/8/8/8/4R3/4RK2 w - - 0 1",
  ),
  fromFen(
    "seventh",
    "Structures",
    "Rook on the seventh",
    "The rook on a7 attacks the pawn on h7.",
    "4k3/R6p/8/8/8/8/8/4K3 w - - 0 1",
  ),
  fromFen(
    "storm",
    "Structures",
    "Kingside storm",
    "King on c1, with f4 and g4 already advanced.",
    "6k1/8/8/8/5PP1/8/8/2K5 w - - 0 1",
  ),
  fromFen(
    "opposite",
    "Structures",
    "Opposite-side attack",
    "King on c1, enemy king on g8, pawns on f4 and g4, rook on the open h-file.",
    "6k1/8/8/8/5PP1/8/8/2K4R w - - 0 1",
  ),
  fromFen(
    "pin",
    "Tactics",
    "Absolute pin",
    "The bishop on b5 pins the knight on c6 to the king on e8.",
    "4k3/8/2n5/1B6/8/8/8/4K3 w - - 0 1",
  ),
  fromFen(
    "fork",
    "Tactics",
    "Knight fork",
    "The knight on d5 attacks the queen on c7 and the rook on f6.",
    "k7/2q5/5r2/3N4/8/8/8/4K3 w - - 0 1",
  ),
  fromFen(
    "skewer",
    "Tactics",
    "King and rook skewer",
    "The bishop on a2 checks the king on b3, with the rook on c4 behind it.",
    "8/8/8/8/2r5/1k6/B7/4K3 w - - 0 1",
  ),
  fromFen(
    "discovery",
    "Tactics",
    "Discovered attack",
    "The knight on e4 can capture on c5 and uncover the rook against the queen.",
    "4q2k/8/8/2p5/4N3/8/8/4R1K1 w - - 0 1",
  ),
  fromFen(
    "queen-pawn",
    "Captures",
    "Queen takes a free pawn",
    "Qxd5 wins a pawn. Nothing recaptures.",
    "4k3/8/8/3p4/8/8/8/3QK3 w - - 0 1",
  ),
  fromFen(
    "queen-hanged",
    "Captures",
    "Queen takes a defended pawn",
    "Qxd5 wins a pawn and loses the queen to the knight on e7.",
    "4k3/4n3/8/3p4/8/8/8/3QK3 w - - 0 1",
  ),
  fromFen(
    "pawn-queen",
    "Captures",
    "Pawn takes the queen",
    "dxe5 wins the queen. The rook on e8 can recapture, and then loses to the rook on e1, so it stops.",
    "4r2k/8/8/4q3/3P4/8/8/4R1K1 w - - 0 1",
  ),
  fromFen(
    "exchange",
    "Captures",
    "Knight takes the queen, pawn follows",
    "Nxf6 wins the queen. exf6 wins the knight back, then gxf6 wins the pawn.",
    "4k3/4p3/5q2/6P1/6N1/8/8/4K3 w - - 0 1",
  ),
  fromFen(
    "cheapest",
    "Captures",
    "Cheapest recapture",
    "Qxd5 wins the rook. Both the pawn on c6 and the queen on d8 attack d5. The pawn recaptures.",
    "3q3k/8/2p5/3r4/8/8/8/3QK3 w - - 0 1",
  ),
  fromFen(
    "en-passant",
    "Captures",
    "En passant, then the rook",
    "exd6 takes the pawn. The rook on d8 recaptures the pawn.",
    "3rk3/8/8/3pP3/8/8/8/4K3 w - d6 0 1",
  ),
  fromFen(
    "mate-capture",
    "Captures",
    "Capture that mates",
    "Qxb7 captures the pawn and checkmates. The knight on c5 defends the queen.",
    "k7/1p6/1Q6/2N5/8/8/8/4K3 w - - 0 1",
  ),
];

export function presetById(id: string): Preset {
  return PRESETS.find((preset) => preset.id === id) ?? PRESETS[0];
}
