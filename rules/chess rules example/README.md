# Forming strategies

The chess program could already list legal moves, keep four turns of history, and look one full turn ahead. Three definitions were still missing: the 20 strategies, how developed each one is, and the gain and loss of a capture.

This project states those definitions and runs them on a position. The written rules are in [docs/definitions.md](docs/definitions.md). The original steps they complete are in [docs/original-rules.md](docs/original-rules.md). The checked implementation is `lib/chess`.

## Run it

```bash
npm install
npm run dev
```

Open [http://127.0.0.1:38471](http://127.0.0.1:38471).

The page loads example positions, scores List 1, prices List 2, and steps one move ahead into Lists 3 and 4. The other tabs are the strategy conditions, the score bands, and the capture formula.

## Test

```bash
npm test
```

The tests cover the score of the starting position, one fully developed example of each strategy, and the capture numbers in the definitions.

## What this does not do

Steps 4 through 7 still ask for four files and a full six-ply search. From the starting position that search is about 124 million nodes. This program keeps the one-full-turn horizon the strategy score needs. It does not write those files or walk every line to depth six.
