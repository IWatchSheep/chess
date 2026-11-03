# Forming Strategies chess (Rust)

A Rust version of the Forming Strategies program. It is a console chess game that follows Steps 1–14 of the rules:

- Select a piece, see its legal moves, choose a square and confirm with Y or N.
- Four game files are kept up to date as you play.
- Every position gets a full six-move search, and Lists 1–4 read it and the history files.
- Lists 1–4 are built from the 20 strategies.

It has **no dependencies**. The move generator, FEN and SAN code that `chess.js` provided in the TypeScript version are written here, so `cargo build` needs no internet access.

## Open it in RustRover

1. **File → Open** and choose this folder (the one with `Cargo.toml`).
2. RustRover detects the Cargo project. Use the green Run arrow next to `fn main` in `src/main.rs`, or run `cargo run --release` in the terminal.
3. To run the tests, right-click the `tests` folder and choose **Run 'All Tests'**, or run `cargo test --release`.

The debug profile is built with optimizations (`opt-level = 2` in `Cargo.toml`) because the six-move search is heavy. Set it to `0` if you want to step through code in the debugger.

## Playing

```
White — select a piece by its square (e.g. e2), or type a command: e2
White pawn on e2 — 2 legal moves: e3, e4
Destination square (Enter to choose a different piece): e4
Move the pawn from e2 to e4 (e4)? Confirm with Y or cancel with N: Y
```

Each step behaves as the rules describe:

- **Step 2:** a piece with no legal moves gets a message saying it cannot currently move.
- **Step 3:** **N** cancels the move and goes back to Step 1, so you can select the same piece or a different one.
- **Castling:** select the king and enter its destination (g1 or c1).
- **Promotion:** you are asked which piece to promote to.

| Command | What it does |
|---|---|
| `lists [w\|b]` | List 1 (forming strategies) and List 2 (strategy captures) |
| `list3` | List 3 for **every** legal next move |
| `list4 [move or all]` | List 4 for every reply to one move, or to every move |
| `future <move>` | List 3 after one next move in full, plus the possible replies |
| `future <move> <reply>` | List 4: strategy captures after that move and reply |
| `strategies`, `strategy <n>` | the 20 strategies and their conditions |
| `files`, `history [w\|b]`, `wait` | the four files, a history file, waiting for the search |
| `new`, `load <fen>`, `presets`, `preset <id>` | new game (clears all four files), load a position or a reference position |
| `board`, `moves`, `fen`, `quit` | |

Command-line options: `--dir <folder>` puts the files somewhere else (the default is `./game_files`). `--fen "<fen>"` starts from a given position. `perft <depth> [fen]` counts move sequences. `search [fen]` runs the six-move search once.

## The four files (Steps 4–7)

| File | Contents |
|---|---|
| `white_history.txt`, `black_history.txt` | That player's previous four turns and the current turn. For each turn, every piece's legal moves before and after the move. |
| `white_future.txt`, `black_future.txt` | The three-full-turn search (six individual moves, every legal sequence) from the latest position where that player is next to move. |

All four files are cleared when a new game starts (`new`, `load`, `preset`). History files are rewritten after every confirmed move.

The future search runs in the background on all CPU cores, so play can continue while it runs. From the start position it is 124,132,536 nodes, with the strategies read through all six moves (see Reading depth below). If a newer position arrives for the same player, the old search is cancelled. In a recorded game the search is always finished before the move is confirmed, so nothing is cancelled.

Each future file records three things:

- the number of sequences at each of the six depths
- the subtree sizes for each first move
- every sequence of up to three moves, one per line

All six depths are fully searched. Only the line-by-line listing stops at three moves: at six moves it would be over 100 million lines (several GB) per file, every turn. The counts show that the full search ran. `LISTED_PLIES` in `src/search.rs` controls the listing depth.

## Step 15: the permanent AI learning archive

At startup, and on `new`, `load` and `preset`, the program asks:

```
Save this game's information to the permanent AI learning files?
Y — Yes
N — No
```

**N** plays normally and changes no saved byte. **Y** records the game in one file, `game_files/ai_learning/game_<id>.txt`, holding both White and Black. The file is only ever appended to, and earlier games are never touched.

- **Before each move is confirmed** in a recorded game, the six-move future search of the position finishes. Its progress is shown while the program waits.
- **Every confirmed move** appends one `MOVE` record with the ply, move number, side and `RECORD` line. The record holds the **four working files copied whole and unchanged**, byte for byte, at the moment the move was recorded:
  - **Past:** the previous turns in both history files.
  - **Present:** the move just played, in the history file of the side that moved.
  - **Future:** the finished six-move future-search file of the position where that side decided.
- **The official lists** are saved with the move: List 1 for both sides, List 2, List 3 for every legal candidate (the played one marked `PLAYED`, every line labelled `searched`) and List 4 for every reply.
- **The game ends** with `GAME END` (result code 1-0, 0-1 or 1/2-1/2, plus how it ended) or `GAME STOPPED` (the player quit or started another game). If the program is killed, the next start appends `GAME STOPPED` to that file and changes no earlier byte.
- **Memory list:** `ai_learning/memory_index.txt` has one line per saved move (game id and ply), in recording order, for games that have an end block. A limited read returns at most 50 memories, and nothing is ever deleted. The list can be deleted and rebuilt from the archive files.
- **General memory bank:** `ai_learning/banks/general.txt` holds references to every saved move of every ended game.
- **Size:** each move stores the whole future-search files, so a recorded game takes several hundred KB per move.
- **Saving is not training.** No neural network is trained yet. Lists 1–4 are made from the working files exactly as before, whether the game is recorded or not.

Commands: `archive` lists every recorded game and the memory list. `archive <game>` lists the game's saved moves. `archive <game> <ply>` shows one saved move.

Commands are matched exactly, so pasted text such as `New game. All four game files were cleared.` runs nothing. `new` asks before stopping a game in progress.

## How it is organized

| File | Port of | Purpose |
|---|---|---|
| `src/board.rs` | chess.js | board, legal moves, FEN, SAN, check, mate, draws |
| `src/geometry.rs` | `geometry.ts` | piece values, geometric attacks |
| `src/position.rs` | `position.ts` | recorded moves, history window |
| `src/exchange.rs` | `exchange.ts` | capture valuation (Step 11 / 14.3) |
| `src/strategies.rs` | `strategies.ts` | the 20 strategies (Step 14.4) |
| `src/evaluate.rs` | `evaluate.ts` | scoring and Lists 1–4 (Steps 8–13) |
| `src/presets.rs` | `presets.ts` | reference positions |
| `src/search.rs` | new | six-move search (Step 6) |
| `src/files.rs` | new | the four files (Steps 4–7) |
| `src/archive.rs` | new | the permanent AI learning archive |
| `src/main.rs` | new | console game (Steps 1–3) |

## Tests

`cargo test --release` runs:

- **The ported TypeScript tests**: starting-position scores, one fully developed position for each of the 20 strategies, and all eight capture examples from Step 14.3.
- **Move-generator checks** against the published counts, including the start-position numbers from Step 6.
- **The stored search and files** (`tests/stored.rs`): Lists 1–4 read from the future and history files give the same rows as the small in-memory reader, the evidence survives a round trip through the file, and the search counts 124,132,536 nodes from the start. The middlegame cases take about a minute.
- **Step 15** (`tests/archive.rs`), the tests listed in rule 15.10:
  - Saved files match the working files byte for byte, with every piece shown before and after.
  - Each saved future is the finished six-move file; from the start it counts 124,132,536 nodes with strategies read to move 5 and 6.
  - A long game keeps its opening, and the saved lists are the official ones.
  - N changes no byte, and games accumulate.
  - A killed game gets `GAME STOPPED`.
  - The memory list, the limit of 50 and the general bank work, and rebuilding gives the same result.
  - Format-1 files still read.
- **Exact speed-ups** (`tests/search_exact.rs`, `tests/fast_paths.rs`).
- **Running time.** With six-move reading, `tests/stored.rs` and the start-position archive test run full searches from the start and from middlegame positions. They take hours on most machines.

## Differences from the TypeScript version

The Rust engine was run side by side with the original TypeScript code on 300 random game positions. Every difference comes from one of three fixes:

1. **No crash when the opponent is in check.** The TypeScript version threw "Null move not allowed when in check" in about 30% of positions (every time the player being evaluated had just given check). Here, the player whose turn it isn't simply has no moves when the side to move is in check.
2. **Fianchetto bishop colour.** The TypeScript version treated b2 as a light square and b7 as a dark one; it's the other way round. The queenside fianchetto now checks for the correct bishop.
3. **En passant counts as a capture.** In chess.js, `isCapture()` is false for en passant, so Lists 2 and 4 silently left those captures out, even though rule 14.3.7 prices them.

## Past, present and future

| | Where it comes from |
|---|---|
| **Past** | The `RECORD` lines in the two history files: the player's previous four turns, plus the current turn when that player has just moved. The lists read these back from disk. |
| **Present** | The position being scored. |
| **Future** | The stored six-move search, read back from the future file of the player to move. |

While the search walks the tree, it reads the 20 strategies for both players on the positions it reaches. For each strategy it keeps the **earliest** individual move, on any legal line, at which progress and at which completion appear. Folded up the tree, that gives one entry for the searched position (List 1), one for every position after a move (List 3), and one for every position after a move and a reply (List 4). The entries are written at the end of each future file. Lists 1–4 read them from there and never start a search.

- The **development score** keeps the written rule: progress or completion "within one full turn" (the next two individual moves). Scores 45, 60, 75 and 100 are unchanged.
- The **horizon** is extra. Each row in List 1 and List 3 also shows the earliest move, anywhere in the stored search, at which the strategy reaches progress and completion ("completion by move 4"). It does not change the score.

`lists`, `list3`, `future` and `list4` wait for the search of the current position to finish.

### Reading depth (`READ_PLIES` in `src/search.rs`)

All six individual moves are searched and counted: 124,132,536 nodes from the start. **The 20 strategies are read for both players on every position through all six moves** (rules 15.1 and 15.10). Two exact speed-ups make this practical without changing any result:

- **Skipping readings that can't change anything.** A strategy is read on a position only when a stored position above it doesn't yet know that strategy's earliest progress or completion at that depth.
- **Reading each distinct position once.** The 19 strategies that look only at the board are read once per distinct position and reused when move orders transpose. Central pawn break's completion reads the history window, so it is checked on every line.

Strategies also read faster now, with the same results: attacks are tested directly, the search asks only for the three flags, and Promotion only looks for promoting moves when a pawn is on the seventh rank.

`tests/search_exact.rs` proves the stored evidence equals reading every strategy on every position (`full_search_reference`). `tests/fast_paths.rs` proves the faster readings give the same answers as the originals.

**Expect long searches.** On a 2-core machine the start-position search takes about 10 minutes, about 5 minutes on 12 threads. Middlegame positions have about 1–1.5 billion sequences (8–12 times the start), so a middlegame search can take most of an hour. A recorded game waits for it before each move. The shared position cache uses up to about 1 GB of memory.

## Which rules this follows

`rules/created_by_the_user_original_rules.txt` is the original and is what this program follows: a six-individual-move search, with List 3 reusing it. `rules/created_by_the_user_original_rules_and_interpreted_by_grok_and_claude.txt` is a later rewrite that cuts the search to four moves. It is not followed. `rules/definitions.md` holds the strategy, score and capture definitions.

## Fixes made against the written rules

- Strategy 12 (Backward-Pawn Pressure) checks whether a pawn of the pawn's own side defends the **advance square**. It used to check the pawn's own square.
- Strategy 9 (Seventh-Rank Invasion) links captures for any rook or queen on relative rank 5 or higher. It used to link only pieces already on relative rank 6.
- The history files keep four previous turns plus the current turn only for the player who just moved. The other player keeps four.
- `preset` seeds the history files with the preset's move record, so Central Pawn Break reads the right window.

## Differences from the TypeScript version

1. **No crash when the opponent is in check.** The TypeScript version threw "Null move not allowed when in check" in about 30% of positions. Here, the player whose turn it isn't simply has no moves when the side to move is in check.
2. **Fianchetto bishop colour.** The TypeScript version treated b2 as a light square and b7 as a dark one; it's the other way round.
3. **En passant counts as a capture.** In chess.js, `isCapture()` is false for en passant, so Lists 2 and 4 silently left those captures out, even though rule 14.3.7 prices them.
