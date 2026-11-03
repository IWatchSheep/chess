# Forming Strategies Chess

A console chess program that identifies the chess strategies forming for each player, ranks them, and ranks the captures linked to them. It follows Steps 1–15 below.

## Contents

- [Getting started](#getting-started)
  - [Requirements](#requirements) · [RustRover](#open-it-in-rustrover) · [Terminal](#build-and-run-from-a-terminal) · [In-game commands](#commands-during-a-game) · [Tests](#tests)
  - [Source code](#how-the-source-code-is-organized) · [Performance notes](#performance-notes) · [Which rules this follows](#which-rules-this-follows)
- [Playing a move (Steps 1–3)](#playing-a-move-steps-13)
- [The four working files (Steps 4–7)](#the-four-working-files-steps-47)
- [The four lists (Steps 8–13)](#the-four-lists-steps-813)
- [Definitions (Step 14)](#definitions-step-14)
- [The permanent AI learning archive (Step 15)](#the-permanent-ai-learning-archive-step-15)

---

## Getting started

### Requirements

- Rust 1.83 or newer (`rust-version` in `Cargo.toml`).
- No external crates, so `cargo build` needs no internet access.

### Open it in RustRover

1. **File → Open** and choose the folder that contains `Cargo.toml`.
2. RustRover detects the Cargo project. Click the green Run arrow next to `fn main` in `src/main.rs`.
3. To run the tests, right-click the `tests` folder and choose **Run 'All Tests'**.

The normal (debug) build uses `opt-level = 2` in `Cargo.toml` because the six-move search is heavy. Set it to `0` if you want to step through the code in the debugger.

### Build and run from a terminal

```
cargo build --release
cargo run --release
```

| Command | What it does |
|---|---|
| `cargo run --release` | Play a game. The four working files go to `./game_files`. |
| `cargo run --release -- --dir <folder>` | Put the game files in another folder. |
| `cargo run --release -- --fen "<fen>"` | Start from a given position. |
| `cargo run --release -- perft <depth> [fen]` | Count legal move sequences to a depth (a correctness check). |
| `cargo run --release -- search [fen]` | Run the six-move future search once and print the totals. |
| `cargo run --release --example bench_search -- [fen]` | Time the six-move search on a position. |

### Commands during a game

| Command | What it does |
|---|---|
| `e2` (any square) | Select the piece there, see its legal moves, enter a destination, confirm with Y or N |
| `lists [w\|b]` | List 1 and List 2 |
| `list3` | List 3 for every legal next move |
| `future <move>` | List 3 after one next move, with the possible replies |
| `future <move> <reply>` | List 4 after that move and reply |
| `list4 [move\|all]` | List 4 for every reply to one move, or to every move |
| `strategies`, `strategy <n>` | The 20 strategies, or one in full |
| `files`, `history [w\|b]`, `wait`, `status` | The four files, a history file, waiting for the search, search progress |
| `archive`, `archive <game>`, `archive <game> <ply>` | Recorded games, a game's saved moves, one saved move |
| `new`, `load <fen>`, `presets`, `preset <id>` | Start a new game, load a position, list or load a reference position |
| `resign`, `draw`, `board`, `moves`, `fen`, `help`, `quit` | |

### Tests

```
cargo test --release
cargo test --release -- --ignored
```

The first command runs the normal tests. The second runs the full six-move search from the start position and checks it against the Step 6 counts (124,132,536 nodes).

**Expect long test runs.** Some tests run full six-move searches from the start and from middlegame positions. A single one of these tests can take about 4 hours, so the full suite can take many hours.

### How the source code is organized

| File | Purpose |
|---|---|
| `src/main.rs` | Console game (Steps 1–3), command-line options and in-game commands |
| `src/lib.rs` | Lists the library modules |
| `src/board.rs` | Board, legal move generation, FEN and SAN, check, mate and draws (no outside packages) |
| `src/geometry.rs` | Piece values and geometric attacks (Step 14.1) |
| `src/position.rs` | Recorded moves, the history window and the evaluation context |
| `src/exchange.rs` | Capture valuation (Steps 11 and 14.3) |
| `src/strategies.rs` | The 20 strategies (Step 14.4) |
| `src/evaluate.rs` | Development scores and Lists 1–4 (Steps 8–13) |
| `src/presets.rs` | Reference positions: one fully developed position for each of the 20 strategies |
| `src/search.rs` | The six-move future search (Step 6) |
| `src/files.rs` | The four working files (Steps 4–7) |
| `src/fastio.rs` | Buffered writes, memory-mapped reads, and the binary evidence cache |
| `src/archive.rs` | The permanent AI learning archive (Step 15) |
| `examples/bench_search.rs` | Times the six-move search on a position |
| `rules/` | The written rules (`rules.txt`) and the user's notes |

| Test file | What it checks |
|---|---|
| `tests/perft.rs` | Move generation against published counts, including the Step 6 start-position numbers |
| `tests/rules.rs` | Chess rules (castling, en passant, promotion, check, mate, draws) and random games that check the program never crashes |
| `tests/strategies.rs` | Development scores, all 20 strategies, and strategy captures |
| `tests/exchange.rs` | The capture valuation examples from Step 14.3 |
| `tests/stored.rs` | Lists 1–4 read from the stored future and history files |
| `tests/search_exact.rs` | The stored search evidence equals reading every strategy on every position |
| `tests/fast_paths.rs` | The faster strategy readings give the same answers as the originals |
| `tests/archive.rs` | The Step 15 archive tests listed in rule 15.10 |
| `tests/file_speed.rs` | Buffered, memory-mapped and binary-cache reads. Does not run the six-move search |

### Performance notes

The search settings are constants at the top of `src/search.rs`:

| Constant | Value | Meaning |
|---|---|---|
| `SEARCH_PLIES` | 6 | Search depth: three moves by each player (Step 6) |
| `READ_PLIES` | 6 | The 20 strategies are read on every position through all six moves |
| `LISTED_PLIES` | 3 | Sequences up to this length are written to the future file one per line |

- **The whole search runs, and only the listing is shortened.** All six moves are searched and counted. Each future-search file records the number of sequences at every depth and the size of each first move's subtree. It writes out every sequence only up to three moves, because listing all six-move sequences would mean over 100 million lines per file, every turn.
- **The search uses every CPU core.** It runs in the background, so play continues while it works. If a newer position arrives for the same player, the old search is cancelled. In a recorded game (Step 15), each move waits for the search to finish before it is confirmed.
- **Two speed-ups change no result:**
  - A strategy is read on a position only when the reading could still change a stored result.
  - The 19 strategies that look only at the board are read once for each distinct position and reused when the same position comes from a different move order. Central Pawn Break's completion reads the history window, so it is checked on every line.
  - `tests/search_exact.rs` and `tests/fast_paths.rs` prove both speed-ups give the same answers.
- **Memory:** the shared position cache holds up to 8 million positions (`CACHE_LIMIT` in `src/search.rs`) in 64 shards. A full shard is emptied and refilled.
- **Expect long searches.** The start position has 124,132,536 nodes. Middlegame positions usually have many more sequences, so their searches take much longer. Lists 1–4 (`lists`, `list3`, `future`, `list4`) wait for the current position's search to finish.

#### Faster file reading

[Step 6](#step-6--three-full-turns-into-the-future) keeps performance work separate from the search: the depth stays six moves and no legal sequence is dropped. [Step 15.5](#155-what-is-saved-at-each-move) keeps the four working files as text, copied whole, with no new format. Binary formats such as Protocol Buffers, FlatBuffers or MessagePack would throw that text away, so the files stay text. Only the way the bytes are moved has changed. The code is in `src/fastio.rs`.

| Technique | What it does |
|---|---|
| Buffered writes | History files, future-search files and archive appends go through a 256 KiB buffered writer. |
| Safe replacement | A future file is written to a temporary file and renamed into place, so nothing reads a half-written search. |
| Buffered reads | History `RECORD` lines and the memory list are read with a buffered reader. A limited memory read stops after 50 lines (rule 15.8) and deletes nothing. |
| Memory-mapped files (Unix) | The future-search file and each archive are mapped into the process. Lists 1–4 parse the strategy evidence from the mapping, without first copying the sequence listing into one big string. |
| Windows | No `mmap`. The same bytes are read through a buffered reader, then go through the same parser and evidence cache. |
| Binary evidence cache | See below. |

**The binary evidence cache**

- Next to `white_future.txt` the program may write `white_future.txt.evidence`, a compact copy of the strategy evidence the lists read.
- The cache stores a hash of the text file's bytes and is used only while that hash still matches. Length and modification time alone are not trusted, so a same-size edit is never served from an old cache.
- On a mismatch, the program parses the text and rewrites the cache.
- On a cache hit, the sequence listing is not parsed line by line.
- The cache is not one of the four working files and is not stored in the archive.
- `*.evidence` files are listed in `.gitignore`, so they are never committed.

### Which rules this follows

`rules/rules.txt` is the specification this README follows.

The earlier note in `rules/users note about archive file handling.txt` said memories past the cap are dropped, and that this program sorts useful memories to the front. [Rule 15.8](#158-memory-list-and-limit) keeps every memory and leaves that sorting to a trained network. The program follows rule 15.8.

---

## Playing a move (Steps 1–3)

**Step 1 — Select a piece.** When you select a piece, the program shows all of its legal moves.

**Step 2 — Pieces with no legal moves.** If the selected piece has no legal moves, the program says the piece cannot currently make a legal move. Such pieces are still considered when identifying forming strategies (Step 8). Being unable to move does not stop a piece from contributing to a strategy.

**Step 3 — Move and confirm.**

1. If the piece has legal moves, enter the destination's two-character coordinate (for example `e4`) and press Enter.
2. You are asked to confirm the move:
   - **Y** finalizes the move, and the piece moves to the square.
   - **N** cancels the move. The piece stays where it was, and the program returns to Step 1 so you can select the same piece or a different one.
3. Each time a piece is selected, its legal moves are checked again. If it has none, Step 2 applies and you can select another piece.

Nothing on the board changes until a legal move is confirmed with **Y**.

---

## The four working files (Steps 4–7)

| File | Contents |
|---|---|
| White history file | White's previous four turns and current turn. For each turn, every piece on the board and its legal moves before and after the move. |
| Black history file | The same for Black. |
| White future-search file | The three-full-turn search from positions where White is next to move. |
| Black future-search file | The three-full-turn search from positions where Black is next to move. |

### Steps 4 and 5 — History files

The legal moves of every piece on the board are listed before and after each move is finalized. These lists are kept in two files, one for White's turns and one for Black's. Each file keeps that player's previous four turns and the current turn. The two files also serve as the history files; no other files are needed.

### Step 6 — Three full turns into the future

The program searches three full turns ahead: three moves by White and three by Black, six individual moves in total. Every legal move at every step is examined, including all of the opponent's replies.

From the standard starting position:

| Individual move | Side | Sequences |
|---|---|---|
| 1 | White | 20 |
| 2 | Black | 400 |
| 3 | White | 8,902 |
| 4 | Black | 197,281 |
| 5 | White | 4,865,609 |
| 6 | Black | 119,060,324 |
| **Total** | | **124,132,536 search nodes** |

These counts change as the game progresses.

- The White future-search file holds the search from positions where White is to move; the Black file holds it from positions where Black is to move.
- Each search covers both players' moves, not only the moves of the file's player.
- Both files are updated as the game goes on, so they match the current position.
- **The strategies are read on every position the search reaches, through all six individual moves.** Counting lines to six moves while reading strategies only to four or five is not this search.

**Search depth for Lists 1 and 3**

- List 1 uses the current position, the previous four turns, the current turn, and one full turn of future information.
- List 3 uses the same Step 6 search but starts one individual move into the future. For every legal first move it takes the resulting position, continues along the sequences Step 6 already generated, and uses the remaining five individual moves. Each strategy stays tied to the move and position that produced it.
- List 3 never starts a new six-move search. Six individual moves is the limit for both lists; the search is never extended to seven or eight.
- The same results are used for the captures in List 4.

**Performance and file management** are handled separately. They must never reduce the search depth or leave out legal move sequences.

### Step 7 — Keeping the files organized

The history and future-search files are managed automatically so they stay organized, clean and current. At the start of every new game all four files are cleared and the information for the new game is generated.

Clearing the four working files never clears the Step 15 archive. When a game is being recorded, Step 15 copies the four files before Step 7 discards or overwrites anything. Step 15 does not change how the four files are written.

---

## The four lists (Steps 8–13)

| List | Name | Made from | Sorted by |
|---|---|---|---|
| 1 | Forming Strategies | History file + future-search file (one full turn) + the 20 strategies | Development (Step 9) |
| 2 | Strategy Captures | Strategies in List 1 | Capture value (Step 11) |
| 3 | Future Forming Strategies | Future-search file, one individual move ahead, for every legal next move | Development (Step 9) |
| 4 | Future Strategy Captures | Strategies in List 3 | Capture value (Step 11) |

### Step 8 — List 1: Forming Strategies

List 1 combines:

1. The player's history file (previous four turns and current turn).
2. The player's future-search file, looking one full turn ahead.
3. The 20 strategies.

The result is the list of strategies beginning to form. List 1 is made from those files — not from a FEN, and not from any record created by Step 15.

After List 1 is made, a trained network may read the Step 15 archive and add a second, clearly labelled assessment beside it. It does not replace List 1, does not change the scores 30, 45, 60, 75 and 100, and does not change which strategies are listed.

### Step 9 — Sorting List 1

Most fully developed strategy at the top, least developed at the bottom (see 14.2).

### Step 10 — List 2: Strategy Captures

For each strategy in List 1, List 2 records:

1. The strategy's name.
2. The player's piece that can capture.
3. The opposing piece that can be captured.
4. The squares of the capturing piece and the opposing piece.
5. The potential cost of losing the capturing piece as a result.

### Step 11 — Sorting List 2

Each capture's value considers the captured piece's value, the capturing piece's value, and the risk of losing the capturing piece — gain against loss. List 2 is sorted from most to least valuable capture (see 14.3).

### Step 12 — List 3: Future Forming Strategies

Using the player's Step 6 future-search file, the program looks one individual move ahead and repeats the Step 8 process at that position. This is done for **every** legal next move, and each result stays tied to the move and position that produced it. Each candidate's List 3 is sorted as in Step 9.

List 3 lets you compare strategies after the next move with the ones in List 1. It reads the Step 6 file; it does not start its own search and does not read a rearranged archive instead.

After each candidate's List 3 is made, a trained network may add a second assessment to that candidate only. Results for different moves are never merged, and the original List 3 rows stay as Steps 12 and 14 produced them.

### Step 13 — List 4: Future Strategy Captures

List 4 repeats Steps 10 and 11 using the strategies from List 3. For each strategy it records the same five items as List 2 and is sorted from most to least valuable capture.

- List 2 ranks captures for the current strategies (List 1).
- List 4 ranks captures for the future strategies (List 3).

---

## Definitions (Step 14)

### 14.1 General rules

1. Each strategy is evaluated separately for White and Black.
2. Relative rank 0 is the player's back rank; relative rank 7 is the promotion rank.
3. Recognition uses geometric attacks, so a piece with no legal move can still contribute.
4. Capture lists consider only legal moves, and only when it is the evaluated player's turn.
5. The history window is the player's previous four turns, plus the current turn when that player has just moved.
6. One full turn is two individual moves (two plies).
7. A strategy is listed only when its foundation, progress or completion conditions are met.
8. Future analysis can raise a score to 45 or 75 when the right conditions occur within the next full turn.
9. A strategy completed only in a future sequence stays partially developed in the current evaluation.

### 14.2 Development scoring (Lists 1 and 3)

| Condition | Stage | Score |
|---|---|---|
| Completion met now | Fully developed | 100 |
| Foundation and progress now; completion possible within one full turn | Partially developed | 75 |
| Foundation and progress now; no completion that turn | Partially developed | 60 |
| Foundation now; progress within one full turn | Beginning to form | 45 |
| Foundation only, or progress only | Beginning to form | 30 |

Sort order: fully developed, partially developed, beginning to form; then score (highest first); then strategy name.

From the standard starting position, White's List 1 contains only **Central Pawn Occupation** and **King Shelter**, each scoring 45.

### 14.3 Piece values and capture calculations

| Piece | Value |
|---|---|
| Pawn | 100 |
| Knight | 320 |
| Bishop | 330 |
| Rook | 500 |
| Queen | 900 |
| King | no capture value (10,000 only for ranking in Fork and Skewer) |

- **Expected gain:** total value of opposing pieces the player captures in the exchange.
- **Potential loss:** total value of the player's pieces the opponent captures in the exchange.
- **Net capture value = expected gain − potential loss.**
- Checkmate adds 100,000 to expected gain if the player delivers it, or to potential loss if the opponent's recapture delivers it.

Lists 2 and 4 are sorted by: highest net value, highest expected gain, lowest potential loss, strategy name, SAN, victim square. The same move may appear more than once if several strategies are linked to it.

**Exchange rules**

1. The first capture is always played.
2. Each later recapture may be declined.
3. Only the cheapest legal recapturing piece is considered.
4. A promoting pawn still counts as a pawn when choosing the cheapest recapturer.
5. The exchange stops when continuing would give an equal net value.
6. Promotion adds the promoted piece's value minus 100.
7. En passant gives the captured pawn a value of 100; later recaptures target the capturing pawn's landing square.
8. A capture belongs to a strategy when the capturing piece takes part in it and the captured piece is on one of its target squares.
9. Kings are never capture targets.

**Examples**

| Exchange | Gain | Loss | Net |
|---|---|---|---|
| Queen takes an undefended pawn | 100 | 0 | +100 |
| Queen takes a pawn, knight takes the queen | 100 | 900 | −800 |
| Pawn takes a queen; recapture would lose a rook | 900 | 0 | +900 |
| Knight takes a queen, pawn recaptures, our pawn takes that pawn | 1,000 | 320 | +680 |
| Queen takes a rook; a pawn is the cheaper recapturer | 500 | 900 | −400 |
| En passant, then a rook recapture | 100 | 100 | 0 |
| Queen captures and delivers checkmate | 100,100 | 0 | +100,100 |
| Pawn promotes to a queen and the queen is captured | 800 | 900 | −100 |

### 14.4 The 20 strategies

| # | Strategy | Foundation | Progress | Completion |
|---|---|---|---|---|
| 1 | Central Pawn Occupation | A friendly pawn is on the d- or e-file. | A friendly pawn is on d4 or e4 (White) / d5 or e5 (Black). | Both central squares hold friendly pawns, and both are defended. |
| 2 | Central Pawn Break | The opponent has a central pawn, and a friendly pawn is on a file c–f. | A friendly pawn attacks an opposing central pawn. | The turn window contains a pawn capturing a pawn on the c-, d-, e- or f-file. |
| 3 | Kingside Pawn Storm | At least two friendly pawns on the f-, g- or h-file, at least one has moved, and the king is outside those files. | A participating pawn reaches relative rank 2. | Two participating pawns reach relative rank 3 or higher, and the king is on a file a–d. |
| 4 | Queenside Minority Attack | Fewer pawns than the opponent on the a-, b- and c-files, with at least one there. | A friendly queenside pawn reaches relative rank 2. | A participating pawn reaches relative rank 3, and an opposing pawn on files a–d is isolated or doubled. |
| 5 | King Shelter | King on e1/e8 with a castling right, or already on c1, g1, c8 or g8. | The king has castled, or at most one piece is between the king and a castling rook. | King on a castled square, not attacked, and its three shelter pawns unmoved. |
| 6 | Opposite-Side Castling Attack | Friendly king on the home rank on files a–c or f–h; opposing king on the other wing. | A friendly pawn on the opposing king's wing reaches relative rank 2. | That pawn reaches relative rank 3, and a friendly rook or queen is on a half-open file on that wing. |
| 7 | Fianchetto | The b- or g-pawn has advanced one square, or a friendly bishop is on b2, g2, b7 or g7. | A friendly bishop is on one of those squares. | The bishop attacks at least one of d4, d5, e4, e5. |
| 8 | Open-File Control | At least one file has no friendly pawn, and a friendly rook or queen remains. | A friendly rook or queen is on a file with no friendly pawn. | A fully open file has more friendly major pieces than opposing ones. |
| 9 | Seventh-Rank Invasion | A friendly rook or queen exists, and at least one file has no friendly pawn. | A friendly major piece reaches relative rank 5. | A friendly rook or queen reaches the opponent's second rank and attacks an opposing piece or pawn. |
| 10 | Passed-Pawn Advance | A friendly pawn has no opposing pawn ahead on its file or either adjacent file. | The passed pawn reaches relative rank 3. | The passed pawn reaches relative rank 5 or higher and is defended. |
| 11 | Isolated-Pawn Attack | An opposing pawn has no friendly neighbouring pawn on an adjacent file. | A friendly piece attacks it, or a friendly rook or queen shares its file. | Attacks on the isolated pawn exceed its defences. |
| 12 | Backward-Pawn Pressure | An opposing pawn's advance square is attacked by a friendly pawn, not defended by an opposing pawn, and on a file with no friendly pawn. | A friendly rook or queen is on that pawn's file. | That major piece stays on the file, and the advance square is occupied or attacked. |
| 13 | Knight Outpost | A square on relative ranks 3–6 is empty or holds a friendly knight, is defended by a friendly pawn, and can no longer be attacked by an opposing pawn. | A friendly knight occupies or attacks it. | A friendly knight occupies the outpost. |
| 14 | Bishop Pair | The player has two bishops; the opponent has fewer than two. | At least one friendly bishop has left the back rank. | At least one file c–f has no pawns, and a friendly bishop has three squares of unobstructed travel. |
| 15 | Pin | A friendly bishop, rook or queen lines up with an opposing front piece and a more valuable rear piece or king. | The line to the front piece is unobstructed. | The rear piece is the king, or a second friendly piece also attacks the front piece. |
| 16 | Discovered Attack | Exactly one friendly piece stands between a friendly sliding piece and an opposing non-pawn. | The intervening piece has a legal move off the line. | That move captures or gives check, while the slider still attacks the original target. |
| 17 | Fork | A piece's square or a legal move attacks two opposing non-pawns. | Two opposing targets are attacked now. | Both targets meet the value conditions. For a sliding piece, targets qualify only if they are kings, undefended, or worth at least as much as the attacker. |
| 18 | Skewer | A friendly slider lines up with two opposing pieces, the nearer worth at least as much as the farther. | The line to the front piece is clear, and it is the king or strictly more valuable than the rear piece. | The front piece is the king, or worth more than the slider while the rear piece is undefended or worth at least as much as the slider. |
| 19 | Major-Piece Battery | Two friendly rooks/queens share a rank or file, at most two pieces between them, and at least one has moved. | At most one piece between them. | The line between them is clear, and they share a half-open file or one attacks an opposing piece along that line. |
| 20 | Promotion | A friendly pawn reaches relative rank 4 or higher. | The pawn reaches relative rank 5 or higher. | The pawn has a legal promotion move. A pawn blocked on the seventh rank stays at progress until it can legally promote. |

### 14.5 Applying the definitions

The same recognition and capture rules are used for all four lists so current and future strategies can be compared.

- **List 1:** strategies forming now, using the move history and one full turn of future analysis.
- **List 2:** legal captures linked to List 1, ranked by capture value.
- **List 3:** recognition repeated one individual move ahead; each strategy stays tied to its move and resulting position.
- **List 4:** captures for List 3's strategies, evaluated after one opposing reply, when the original player is to move again.

### 14.6 How the steps fit together

Steps 4–7 make and maintain the four working files. Steps 8–13 make Lists 1–4 from them. Step 14 supplies the definitions. Step 15 keeps copies of the files Steps 4–6 already wrote; it does not replace them or give them a new format. The search is always three full turns (six individual moves), and List 3 reuses it without extending or shortening it.

---

## The permanent AI learning archive (Step 15)

### 15.1 Why it exists

The working files keep only four past turns, the current turn and the future search; anything older is deleted. Saved only at the end, they would hold the ending but not the opening or middlegame. Step 15 keeps every recorded move of both sides, for the whole game, in one archive file. Each move is kept as three linked parts taken from files Steps 4–6 already wrote:

- **Past:** the four past turns
- **Present:** the current turn
- **Future:** the finished Step 6 future-search file (not a four- or five-ply reading)

Every piece appears in each part, because the history files already list every piece of both colours with its legal moves before and after the move. Each move's past includes the turn that was just the present, so past, present and future link across the whole game.

A FEN is never used in place of these files. Rebuilding the listings or six-ply evidence from a FEN was tried and did not reproduce the files.

Step 15 changes nothing about the chess rules, the 20 strategies, the scores, the six-ply search, or how Steps 4–6 write the files.

### 15.2 Working files are unchanged

Recording does not change how the four working files are written, updated or cleared. Lists 1–4 are still made from them. With recording off, the game plays normally.

### 15.3 The question at the start of every game

At startup, and on `new`, `load` and `preset`, the program asks:

```
Save this game's information to the permanent AI learning files?
Y — Yes
N — No
```

- **Y** saves this game. Before each move is confirmed, the Step 6 search of the position finishes, so the copied future file is the finished six-ply file. Search progress is shown while waiting.
- **N** saves nothing and does not delete any saved game. Confirmation does not wait.
- `perft` and `search` do not ask.

### 15.4 One file per game

Each recorded game has one append-only file, `ai_learning/game_<id>.txt`, inside the game-files directory, holding both White and Black. A game is never split across files. Saved games are not committed to Git (the game-files directory is already excluded).

The file starts with a `GAME START` block and ends with `GAME END` when the game has a result, or `GAME STOPPED` when the player quits or starts another game. If the program is killed, the next start appends `GAME STOPPED` without changing any earlier byte.

### 15.5 What is saved at each move

Each confirmed move appends one record before Step 7 changes the working files. It is marked with the ply, move number, side that moved, and the history file's `RECORD` line. Its three parts are copied unchanged:

1. **Past:** each side's four past turns as the history files hold them (as many as exist).
2. **Present:** the current turn, from the history file of the side that moved.
3. **Future:** the finished Step 6 file for the position where the player decided — six individual moves, strategies read through all six, both players' moves.

Copying the two history files and two future-search files whole satisfies all three parts; their text is not reformatted.

Also saved with the move:

- the official **List 1** for both sides
- the official **List 3** for every legal candidate of the side that moved, each tied to its candidate. The played candidate is marked `PLAYED`; searched lines are labelled `searched`, so they are never stored as played moves.

### 15.6 What is not done

- No FEN stored instead of a history or future-search file.
- No legal-move listing or search rebuilt from a FEN.
- Not just the last window of the game — the opening and middlegame stay.
- No change to the 20 strategies, scores or six-ply search.
- A four- or five-ply reading is not the Step 6 file.

### 15.7 How Lists 1 and 3 use the archive

During play, Lists 1–4 are made exactly as Steps 8–14 say, from the working files.

After List 1 exists, a trained network may read the archive and add a separate, labelled assessment beside it; after each List 3 candidate exists, it may add one to that candidate only. Original rows, scores and order stay. Memories never cause a strategy to be listed — that is decided only by the 20 definitions, the current position and the current game's history window.

Saving is not training. Training happens only when a network's weights are updated, in the separate AI repository. With no archive or no trained network, Lists 1 and 3 work as normal and the added assessment says **AI advice is unavailable**.

### 15.8 Memory list and limit

All saved moves form one list called memory, kept in `ai_learning/memory_index.txt`. Each line points to one saved move by game id and ply. A game enters the list only after it has a `GAME END` or `GAME STOPPED` block, so a game never reads its own later moves or result. The index can be deleted and rebuilt from the archive files without changing them.

The list stays in recording order until a trained network writes an order; the network may move useful memories to the front. The chess program does not decide usefulness, and sorting deletes nothing.

A limited read returns at most **50** memories. The limit can be changed later in the AI repository and never deletes a memory.

### 15.9 Memory banks

A memory bank is a named list of references to saved moves, not a copy. Banks live in `ai_learning/banks/`. The chess program creates one, `general.txt`, with every saved move from every game that has an end block.

A task may use a more specific bank while forming Lists 1 and 3, then return to the general bank. Specific banks (including ones for other tasks such as solved grid-board puzzles) and when to switch belong to the separate AI repository. Chess lists read chess banks. Creating a bank does not change the four working files.

### 15.10 Tests

- A recorded move's saved history and future-search files match the working files at that moment, byte for byte.
- Each saved history file lists every piece of both colours with legal moves before and after the move.
- Each saved future file is the finished six-ply file; from the start position it counts 124,132,536 nodes with strategies read through all six moves.
- A game longer than the rolling window still has its first move (the opening), middlegame and endgame in the archive.
- One archive file per game, with both White and Black; every move has a past, present and future.
- **N** changes no saved byte. A new game clears the four working files but not the archive. Several recorded games accumulate.
- A killed game gets `GAME STOPPED` on the next start, with earlier bytes unchanged.
- Lists 1–4 are identical whether or not the game is recorded. Any network assessment stays tied to the right player, position and candidate.
- The memory index rebuilds to the same result; a game is absent until it ends; the limit returns at most 50 and deletes nothing; the general bank contains the finished games.
- No test passes by rebuilding a file from a FEN.
- Every earlier test still passes.
