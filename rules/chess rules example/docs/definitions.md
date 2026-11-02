# The three missing definitions

These rules complete the program described in [original-rules.md](original-rules.md). They define the 20 strategies, the development score that sorts List 1 and List 3, and the gain and loss that sort List 2 and List 4.

The reference implementation is `lib/chess`. `npm test` checks the score bands, one full example of each strategy, and the capture numbers below.

## How a position is read

Strategies are scored for one side at a time.

**Relative rank.** 0 is the side's own back rank and 7 is its promotion rank. A white pawn starts at relative rank 1. Relative rank 2 is White's third rank, or Black's sixth. Relative rank 3 is White's fourth rank. Relative rank 4 is the first rank of the opponent's half. Relative rank 5 is White's sixth rank. The opponent's second rank, the invasion rank, is relative rank 6.

**Geometric attack.** A piece attacks a square by its move shape. Pawns attack one square diagonally forward. Sliders stop at the first piece. Pins, checks, and whose turn it is do not matter. A piece with no legal move still counts. This is the rule from Step 2.

**Legal move.** A move that chess rules allow on that side's turn, including king safety. Promotion is a separate legal move for each piece the pawn can become. When it is not that side's turn, "has a legal move" is answered by flipping the turn and generating moves as if the opponent passed. Capture lists do not use that flip. A capture is priced only when it is actually that side's turn, because the opponent's reply is part of the loss.

**Turn window.** The previous four turns by the side being scored. If that side has just moved, the current turn is included, so the window holds up to five of that side's moves. Fewer moves are used at the start of the game. The central pawn break is the strategy whose completion is read from this window rather than from the board alone.

**One full turn.** Two plies, every legal move, both sides. If the scored side is to move, that is its move and the reply. If the opponent is to move, that is the opponent's move and the scored side's reply. A future condition counts when it is true after the scored side's move in that span, or still true after the following reply. The search stops when a line ends in checkmate. This is the horizon List 1 uses. It is not a new six-ply search, and List 3 must not start one either. List 3 applies the same reader to the position one ply later, inside the search Step 6 already built.

A strategy is listed only when foundation or progress is already true on the position being scored, or completion is already true. A pattern that exists only in a future line is not a row.

## Development score

Each listed strategy gets one score. List 1 and List 3 sort by stage, then by score, then by name. Fully developed comes before partially developed, which comes before beginning to form.

| What is true now | Stage | Score |
| --- | --- | --- |
| Completion | Fully developed | 100 |
| Foundation and progress, and completion occurs within one full turn | Partially developed | 75 |
| Foundation and progress, and completion does not | Partially developed | 60 |
| Foundation, progress only within one full turn | Beginning to form | 45 |
| Foundation only, or progress only | Beginning to form | 30 |

Completion on the position being scored is always fully developed, even if a future line is what would have been required a move earlier. Reaching completion inside the next full turn does not make the strategy fully developed yet. That is why 75 stays partial.

On the starting position, for White:

- Central pawn occupation is beginning, score 45. The d- and e-pawns are the foundation, and `d4` or `e4` is legal within one full turn.
- King shelter is beginning, score 45. The king is on `e1` with castling rights, and one developing move leaves at most one piece between the king and a rook.
- Nothing else is listed. The queen sitting behind the d-pawn is not a discovered attack, because the piece behind the screen is a pawn.

## Capture valuation

Piece values, in the units used everywhere below:

| Piece | Value |
| --- | ---: |
| Pawn | 100 |
| Knight | 320 |
| Bishop | 330 |
| Rook | 500 |
| Queen | 900 |

A king has no capture value. The comparison value 10000 is used only to decide that a king is heavier than every other piece inside fork and skewer rules. It is never added to gain or loss.

**Expected gain** is the total value of opposing pieces the original player takes in the priced sequence.

**Potential loss** is the total value of the original player's own pieces the opponent takes in that same sequence.

**Net** is expected gain minus potential loss. List 2 and List 4 sort by net, highest first. Ties go to the higher gain, then the lower loss, then strategy name, then the move's notation.

The capture being scored is always played. Every recapture after it may be declined.

1. Play the capture. Add the victim's value to expected gain. An en passant victim is a pawn, and later recaptures are aimed at the square the capturing pawn landed on, not the square the pawn left.
2. A pawn that promotes on the capture adds the new piece's value minus 100 to expected gain. The piece now standing on the square is the promoted piece, so a later recapture loses the queen, rook, bishop, or knight, not the pawn.
3. If the move checkmates, add 100000 to expected gain and stop. Potential loss stays 0.
4. Otherwise the opponent, then the original player, may recapture on that same square.
5. Only legal captures onto the square count, so a pinned piece that would have to leave the pin is not a recapturer.
6. Among those recaptures, only pieces of the cheapest standing value are tried. A pawn is worth 100 even when that pawn will promote. A king, if it is the only legal recapturer, is tried and is never preferred over a cheaper piece. When several pieces share the cheapest value, each is tried.
7. The side about to recapture keeps the result that is best for it. The original player maximizes net. The opponent minimizes the original player's net. If stopping ties the best recapture, the side stops.
8. If a recapture checkmates, add 100000 to expected gain when the original player mates, or to potential loss when the opponent mates, and stop.

A capture is attached to a strategy only when both are true in the position being scored: the moving piece is one of that strategy's participating pieces, and the captured piece stands on one of that strategy's target squares. The same move is repeated once for each strategy that claims it. A strategy with no such move is still listed. It simply adds no capture row.

Checked numbers:

| Capture | Expected gain | Potential loss | Net | Why |
| --- | ---: | ---: | ---: | --- |
| Queen takes an undefended pawn | 100 | 0 | +100 | Nothing recaptures. |
| Queen takes a pawn, knight recaptures the queen | 100 | 900 | −800 | The queen is lost. |
| Pawn takes a queen, and the recapture would lose a rook | 900 | 0 | +900 | The opponent stops. |
| Knight takes a queen, pawn recaptures, our pawn takes that pawn | 1000 | 320 | +680 | Both sides continue because each recapture helps them. |
| Queen takes a rook, pawn recaptures, queen also could | 500 | 900 | −400 | The pawn is the cheaper recapturer. |
| En passant, rook recaptures the pawn | 100 | 100 | 0 | The pawn is won and lost. |
| Queen captures and checkmates | 100100 | 0 | +100100 | Mate bonus plus the pawn. |
| Pawn promotes to a queen and the new queen is taken | 800 | 900 | −100 | Promotion gain 800, then the queen is lost. |

## The 20 strategies

Participating pieces are the pieces that may make a strategy's capture. Targets are the opposing pieces those captures may take. Kings are never targets, because a king is not a legal capture.

### 1. Central pawn occupation

Occupy `d4` and `e4`, or `d5` and `e5`, with pawns the rest of the army defends.

- **Foundation.** At least one friendly pawn stands on the d-file or the e-file.
- **Progress.** A friendly pawn occupies the side's central squares: `d4` or `e4` for White, `d5` or `e5` for Black.
- **Completion.** Friendly pawns occupy both of those squares, and each is geometrically defended.
- **Captures.** The capturer is a central pawn or a piece defending one. The victim stands on `c4`–`f5` or is a pawn attacking a friendly central pawn.

### 2. Central pawn break

Challenge an opposing central pawn with a c-, d-, e-, or f-pawn capture.

- **Foundation.** The opponent has a pawn on `d4`, `d5`, `e4`, or `e5`, and a friendly pawn remains on the c-, d-, e-, or f-file.
- **Progress.** A friendly pawn geometrically attacks an opposing pawn on one of those four squares.
- **Completion.** The turn window contains a friendly pawn capturing a pawn on the c-, d-, e-, or f-file.
- **Captures.** The capturer is a c–f pawn. The victim is an opposing central pawn that pawn attacks.

### 3. Kingside pawn storm

Advance the f-, g-, and h-pawns while the friendly king stays off that wing.

- **Foundation.** At least two pawns remain on f–h, at least one has left its starting square, and the king is not on f, g, or h.
- **Progress.** A pawn on f, g, or h has relative rank 2 or more, and the king is still off f–h.
- **Completion.** At least two of those pawns have relative rank 3 or more, and the king is on files a–d.
- **Captures.** The capturer is a storm pawn or a piece attacking the f–h wing. The victim stands on f, g, or h.

### 4. Queenside minority attack

Advance a smaller queenside pawn mass until the opponent has an isolated or doubled pawn.

- **Foundation.** On files a–c the opponent has strictly more pawns, the side has at least one, and the opponent has a pawn on a–d.
- **Progress.** A friendly pawn on a, b, or c has relative rank 2 or more.
- **Completion.** A friendly pawn on a–c has relative rank 3 or more, and an opposing pawn on a–d is isolated or doubled.
- **Captures.** The capturer is an a–c pawn or a major piece on a–d. The victim is an opposing pawn on a–d.

### 5. King shelter

Castle, and keep the three pawns in front of the king on their original squares.

- **Foundation.** The king is on `e1` or `e8` with at least one castling right, or the king is already on `c1`, `g1`, `c8`, or `g8`.
- **Progress.** The king is on a castled square, or at most one piece remains between the king and a rook that still has castling rights.
- **Completion.** The king is on `g1`, `g8`, `c1`, or `c8`, is not attacked, and the three shelter pawns on that wing are unmoved.
- **Captures.** The capturer is the king, a shelter pawn, or a back-rank rook. The victim attacks the king or a shelter pawn.

### 6. Opposite-side castling attack

Storm the wing where the enemy king lives after the kings have castled on opposite sides.

- **Foundation.** The friendly king is on the home rank on files a–c or f–h, and the enemy king is on the other wing (files f–h or a–c).
- **Progress.** A friendly pawn on the enemy king's wing has relative rank 2 or more.
- **Completion.** One of those pawns has relative rank 3 or more, and a friendly rook or queen stands on a half-open file of that wing.
- **Captures.** The capturer is a wing pawn or a major piece on that wing. The victim stands on the enemy king's wing.

### 7. Fianchetto

Place a bishop on the long diagonal behind a one-step b- or g-pawn and aim it at the center.

- **Foundation.** The b- or g-pawn has moved exactly one square, or the bishop is already on `b2`, `g2`, `b7`, or `g7`, and that color of bishop still exists.
- **Progress.** The bishop occupies the fianchetto square.
- **Completion.** That bishop geometrically attacks `d4`, `d5`, `e4`, or `e5`.
- **Captures.** The capturer is the fianchetto bishop or the pawn that opened its diagonal. The victim is a piece the bishop attacks.

### 8. Open-file control

Put a rook or queen on a file with no friendly pawn, and own a fully open file.

- **Foundation.** A file has no friendly pawn, and a friendly rook or queen is still on the board.
- **Progress.** A friendly rook or queen stands on a file with no friendly pawn.
- **Completion.** A fully open file contains more friendly rooks and queens than opposing ones.
- **Captures.** The capturer is a rook or queen on a half-open file. The victim is a piece that major piece attacks.

### 9. Seventh-rank invasion

Bring a rook or queen to the opponent's second rank and attack something there.

- **Foundation.** A friendly rook or queen exists, and at least one file has no friendly pawn.
- **Progress.** A friendly rook or queen has relative rank 5 or more.
- **Completion.** A friendly rook or queen is on the opponent's second rank and geometrically attacks an opposing unit.
- **Captures.** The capturer is a major piece on the sixth rank or beyond. The victim is a piece that invader attacks.

### 10. Passed-pawn advance

Create a passed pawn, escort it past the midline, and defend it on the sixth rank or beyond.

- **Foundation.** A friendly pawn has no opposing pawn on its file or an adjacent file in front of it.
- **Progress.** That passed pawn has relative rank 3 or more.
- **Completion.** A passed pawn has relative rank 5 or more and is geometrically defended.
- **Captures.** The capturer is the passed pawn or a piece defending it. The victim blocks it, attacks it, or stands on the square in front of it.

### 11. Isolated-pawn attack

Fix an opposing pawn that has no neighbor, then attack it more times than it is defended.

- **Foundation.** The opponent has a pawn with no friendly pawn on an adjacent file.
- **Progress.** A friendly unit geometrically attacks that pawn, or a friendly rook or queen shares its file.
- **Completion.** The isolated pawn is geometrically attacked more times than it is defended.
- **Captures.** The capturer attacks the isolated pawn or shares its file as a rook or queen. The victim is the isolated pawn or a piece defending it.

### 12. Backward-pawn pressure

Restrain a pawn whose advance square is hit by a friendly pawn, then line a major piece up against it.

- **Foundation.** An opposing pawn's advance square is attacked by a friendly pawn, no opposing pawn defends it, and its file has no friendly pawn.
- **Progress.** A friendly rook or queen shares that pawn's file.
- **Completion.** The advance square is occupied or attacked by the side, and the major piece remains on the file.
- **Captures.** The capturer is the major piece or a unit controlling the advance square. The victim is the backward pawn or a piece defending it.

### 13. Knight outpost

Plant a knight on a square in the opponent's half that a friendly pawn defends and no enemy pawn can attack.

- **Foundation.** Such a square exists on relative ranks 3 through 6. It is empty or already occupied by a friendly knight. An enemy pawn can still attack the square when it stands on an adjacent file and has not yet passed the rank from which a pawn attacks that square.
- **Progress.** A friendly knight occupies the square or geometrically attacks it.
- **Completion.** A friendly knight occupies the square.
- **Captures.** The capturer is the knight or the pawn that defends the outpost. The victim is a piece the occupying knight attacks.

### 14. Bishop pair

Keep both bishops against fewer opposing bishops and open a central file for them.

- **Foundation.** The side has two bishops and the opponent has fewer than two.
- **Progress.** At least one friendly bishop has left the back rank.
- **Completion.** A file from c through f is empty of pawns, and a friendly bishop has at least three squares of clear travel.
- **Captures.** The capturer is one of the two bishops. The victim is a piece that bishop attacks.

### 15. Pin

Align a bishop, rook, or queen against a piece that cannot step aside without exposing a heavier piece or the king.

- **Foundation.** A friendly slider, an opposing front piece, and a heavier opposing rear piece or the king share a line, with at most one non-friendly piece between the slider and the front piece and nothing between the two opposing pieces.
- **Progress.** Nothing remains between the slider and the front piece.
- **Completion.** The line is clear and either the rear piece is the king or a second friendly unit also attacks the front piece.
- **Captures.** The capturer is the pinning slider or the second attacker. The victim is the pinned piece.

### 16. Discovered attack

Move a piece off a slider's line so the slider's attack arrives as the mover captures or gives check.

- **Foundation.** Exactly one friendly piece stands between a friendly slider and an opposing piece other than a pawn.
- **Progress.** That screening piece has a legal move off the line.
- **Completion.** One of those moves captures or gives check, and the slider still attacks the original target afterward.
- **Captures.** The capturer is the slider or the screening piece. The victim is the discovered target.

### 17. Fork

Attack two pieces of at least knight value with one piece, or reach the square that does it.

- **Foundation.** A legal move, or the piece's current square, attacks two opposing units other than pawns. Slider forks count only when both targets are royal, undefended, or at least as valuable as the slider.
- **Progress.** A knight or pawn currently attacks two such pieces, or a slider currently has a qualifying pair.
- **Completion.** Two current targets are each a king, geometrically undefended, or worth at least as much as the forker.
- **Captures.** The capturer is the forking piece. The victim is one of the forked pieces.

### 18. Skewer

Attack a more valuable piece that must step off a line and leave a lesser piece behind it.

- **Foundation.** A friendly slider is aligned with two opposing pieces, the nearer one at least as valuable as the farther one, with at most one non-friendly blocker in front.
- **Progress.** The line to the front piece is clear, and the front piece is the king or strictly more valuable than the piece behind it.
- **Completion.** The front piece is the king, or it is worth more than the slider and the rear piece is undefended or worth at least the slider.
- **Captures.** The capturer is the slider. The victim is the front piece or the piece behind it.

### 19. Major-piece battery

Double rooks, or a rook and queen, on a file or rank that is open or aimed at a target.

- **Foundation.** Two friendly rooks or queens share a file or rank, at most two pieces stand between them, and at least one has left its original square.
- **Progress.** At most one piece stands between them.
- **Completion.** Nothing stands between them, and they share a half-open file or one of them attacks an opposing piece along the line.
- **Captures.** The capturer is one of the battery pieces. The victim is a piece that battery piece attacks.

### 20. Promotion

Escort a pawn into the opponent's half and give it a legal move onto the last rank.

- **Foundation.** A friendly pawn has relative rank 4 or more.
- **Progress.** A friendly pawn has relative rank 5 or more.
- **Completion.** A friendly pawn has a legal move that promotes. A pawn blocked on the seventh rank is progress only. That is the Step 2 case: the pawn still belongs to the strategy, and it is not fully developed until promotion is legal.
- **Captures.** The capturer is the advanced pawn or a piece defending it. The victim attacks that pawn, stands in front of it, or can be taken by it.

## Where the lists use these rules

- **List 1** is every strategy this score lists for the current position, using the turn window and one full turn of the future search.
- **List 2** is the captures attached to those strategies, sorted by net value.
- **List 3** runs the same recognition on the position after one legal move. The strategies stay tied to that move.
- **List 4** runs the same capture rule for the strategies recognized when the scored side is to move again, after one opposing reply. The reply stays tied to those rows.

The two history files and the two future-search files from Steps 4 through 7 are still the program's storage. This reference reads a FEN and the move list in memory instead of writing those files, and it does not expand the six-ply tree of the starting position. The definitions above are the part those files were missing.
