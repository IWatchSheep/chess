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

The future search runs in the background on all CPU cores, so the game never waits for it. From the start position it is 124,132,536 nodes and takes about 1–4 seconds. Middlegame positions can be 10–50 times larger. If a newer position arrives for the same player, the old search is cancelled.

Each future file records three things:

- the number of sequences at each of the six depths
- the subtree sizes for each first move
- every sequence of up to three moves, one per line

All six depths are fully searched. Only the line-by-line listing stops at three moves: at six moves it would be over 100 million lines (several GB) per file, every turn. The counts show that the full search ran. `LISTED_PLIES` in `src/search.rs` controls the listing depth.

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
| `src/main.rs` | new | console game (Steps 1–3) |

## Tests

`cargo test --release` runs:

- **The ported TypeScript tests**: starting-position scores, one fully developed position for each of the 20 strategies, and all eight capture examples from Step 14.3.
- **Move-generator checks** against the published counts, including the start-position numbers from Step 6.
- **The stored search and files** (`tests/stored.rs`): Lists 1–4 read from the future and history files give the same rows as the small in-memory reader, the evidence survives a round trip through the file, and the search counts 124,132,536 nodes from the start. The middlegame cases take about a minute.

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

All six individual moves are searched and counted: 124,132,536 nodes from the start. Strategies are read on the positions up to individual move **4**. Reading them on moves 5 and 6 as well was built and measured, and it is too slow to play with:

| Strategy reading depth | From the start position (12 threads, release) |
|---|---|
| 4 (default) | about 5 s |
| 5 | about 93 s |
| 6 | more than 10 minutes (about 119 million positions read for both players) |

A middlegame has 20 to 50 times more positions, and the search is redone after every move. Set `READ_PLIES` to 5 or 6 if you want the deeper horizon and can wait. Nothing else changes.

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
