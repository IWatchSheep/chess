"use client";

import { useMemo, useState } from "react";
import { Chess } from "chess.js";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ChessBoard } from "@/components/chess-board";
import {
  PIECE_VALUE,
  PRESETS,
  STRATEGIES,
  evaluateFen,
  evaluatePosition,
  formatMaterial,
  formatSigned,
  historyWindow,
  pieceName,
  presetById,
  recordMove,
  sideName,
  type CaptureRow,
  type Color,
  type Evaluation,
  type RecordedMove,
  type Stage,
  type StrategyHit,
  type ValuedCapture,
} from "@/lib/chess";

const GROUPS = ["Opening", "Structures", "Tactics", "Captures"] as const;

export function Studio() {
  const initial = PRESETS[0];
  const [presetId, setPresetId] = useState(initial.id);
  const [fen, setFen] = useState(initial.fen);
  const [draft, setDraft] = useState(initial.fen);
  const [moves, setMoves] = useState<RecordedMove[]>(initial.moves);
  const [side, setSide] = useState<Color>(initial.side);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [nextSan, setNextSan] = useState("");
  const [replySan, setReplySan] = useState("");

  const evaluation = useMemo(() => {
    try {
      return { data: evaluateFen(fen, side, moves), error: null as string | null };
    } catch (error) {
      return {
        data: null,
        error: error instanceof Error ? error.message : "That position could not be read.",
      };
    }
  }, [fen, moves, side]);

  const selected =
    evaluation.data?.hits.find((hit) => hit.id === selectedId) ?? evaluation.data?.hits[0] ?? null;

  const followUp = useMemo(() => buildFollowUp(fen, side, moves, nextSan, replySan), [fen, moves, nextSan, replySan, side]);

  function applyPreset(id: string) {
    const preset = presetById(id);
    setPresetId(preset.id);
    setFen(preset.fen);
    setDraft(preset.fen);
    setMoves(preset.moves);
    setSide(preset.side);
    setSelectedId(null);
    setLoadError(null);
    setNextSan("");
    setReplySan("");
  }

  function applyDraft() {
    try {
      const chess = new Chess(draft.trim());
      setFen(chess.fen());
      setDraft(chess.fen());
      setMoves([]);
      setSide(chess.turn());
      setPresetId("custom");
      setSelectedId(null);
      setLoadError(null);
      setNextSan("");
      setReplySan("");
    } catch (error) {
      setLoadError(error instanceof Error ? error.message : "That FEN could not be loaded.");
    }
  }

  const activePreset = PRESETS.find((preset) => preset.id === presetId);

  return (
    <Tabs defaultValue="position" className="gap-6">
      <TabsList className="h-auto w-full flex-wrap justify-start">
        <TabsTrigger value="position">Position</TabsTrigger>
        <TabsTrigger value="strategies">The 20 strategies</TabsTrigger>
        <TabsTrigger value="scoring">Development score</TabsTrigger>
        <TabsTrigger value="captures">Capture value</TabsTrigger>
      </TabsList>

      <TabsContent value="position" className="flex flex-col gap-6">
        <div className="grid items-start gap-6 lg:grid-cols-[minmax(16rem,26rem)_minmax(0,1fr)]">
          <div className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
              <label htmlFor="preset" className="text-sm font-medium">
                Example position
              </label>
              <select
                id="preset"
                value={presetId}
                onChange={(event) => applyPreset(event.target.value)}
                className="h-9 rounded-lg border border-input bg-card px-2.5 text-sm"
              >
                {presetId === "custom" ? <option value="custom">Custom position</option> : null}
                {GROUPS.map((group) => (
                  <optgroup key={group} label={group}>
                    {PRESETS.filter((preset) => preset.group === group).map((preset) => (
                      <option key={preset.id} value={preset.id}>
                        {preset.name}
                      </option>
                    ))}
                  </optgroup>
                ))}
              </select>
              <p className="text-sm leading-relaxed text-muted-foreground">
                {activePreset?.detail ??
                  "Custom FEN. The central pawn break can be marked complete only when the capture is in the loaded move window."}
              </p>
            </div>

            <form
              className="flex flex-col gap-2"
              onSubmit={(event) => {
                event.preventDefault();
                applyDraft();
              }}
            >
              <label htmlFor="fen" className="text-sm font-medium">
                FEN
              </label>
              <div className="flex flex-col gap-2 sm:flex-row">
                <Input
                  id="fen"
                  value={draft}
                  onChange={(event) => setDraft(event.target.value)}
                  spellCheck={false}
                  aria-invalid={loadError ? true : undefined}
                  aria-describedby={loadError ? "fen-error" : undefined}
                />
                <Button type="submit" className="sm:w-24">
                  Load
                </Button>
              </div>
              {loadError ? (
                <p id="fen-error" className="text-sm text-destructive" role="alert">
                  {loadError}
                </p>
              ) : null}
            </form>

            <div className="flex flex-col gap-2">
              <p className="text-sm font-medium" id="side-label">
                Strategies for
              </p>
              <div className="grid grid-cols-2 gap-2" role="group" aria-labelledby="side-label">
                {(["w", "b"] as const).map((color) => (
                  <Button
                    key={color}
                    type="button"
                    variant={side === color ? "default" : "outline"}
                    aria-pressed={side === color}
                    onClick={() => {
                      setSide(color);
                      setSelectedId(null);
                      setNextSan("");
                      setReplySan("");
                    }}
                  >
                    {sideName(color)}
                  </Button>
                ))}
              </div>
            </div>

            {evaluation.error ? (
              <Card>
                <CardHeader>
                  <CardTitle>Position could not be scored</CardTitle>
                  <CardDescription>{evaluation.error}</CardDescription>
                </CardHeader>
              </Card>
            ) : (
              <>
                <ChessBoard
                  fen={fen}
                  participants={selected?.participants.map((unit) => unit.square) ?? []}
                  targets={selected?.targets.map((unit) => unit.square) ?? []}
                />
                <ul className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
                  <li className="flex items-center gap-1.5">
                    <span className="inline-block size-3 rounded-sm outline outline-2 outline-[oklch(0.78_0.12_155)]" />
                    Participating piece
                  </li>
                  <li className="flex items-center gap-1.5">
                    <span className="inline-block size-1.5 rounded-full bg-[oklch(0.55_0.16_30)]" />
                    Capture target
                  </li>
                </ul>
                <WindowNote fen={fen} moves={moves} side={side} />
              </>
            )}
          </div>

          <div className="flex min-w-0 flex-col gap-4">
            {evaluation.data ? (
              <PositionLists
                evaluation={evaluation.data}
                selectedId={selected?.id ?? null}
                onSelect={setSelectedId}
              />
            ) : null}
          </div>
        </div>

        {evaluation.data ? (
          <FollowUp
            evaluation={evaluation.data}
            followUp={followUp}
            nextSan={nextSan}
            replySan={replySan}
            onNext={(san) => {
              setNextSan(san);
              setReplySan("");
            }}
            onReply={setReplySan}
          />
        ) : null}
      </TabsContent>

      <TabsContent value="strategies" className="flex flex-col gap-4">
        <p className="max-w-3xl text-sm leading-relaxed text-muted-foreground">
          Each strategy is recognized from three conditions. Foundation is the structural base.
          Progress is the committing step already visible on the board. Completion is the finished
          pattern. A piece with no legal move can still satisfy a geometric condition.
        </p>
        <div className="grid gap-3">
          {STRATEGIES.map((strategy, index) => (
            <Card key={strategy.id} size="sm">
              <CardHeader>
                <CardTitle>
                  {index + 1}. {strategy.name}
                </CardTitle>
                <CardDescription>{strategy.intent}</CardDescription>
              </CardHeader>
              <CardContent className="flex flex-col gap-2 text-sm leading-relaxed">
                <p>
                  <span className="font-medium">Foundation. </span>
                  {strategy.foundation}
                </p>
                <p>
                  <span className="font-medium">Progress. </span>
                  {strategy.progress}
                </p>
                <p>
                  <span className="font-medium">Completion. </span>
                  {strategy.completion}
                </p>
                <p className="text-muted-foreground">
                  <span className="font-medium text-foreground">Captures. </span>
                  {strategy.captureLink}
                </p>
              </CardContent>
            </Card>
          ))}
        </div>
      </TabsContent>

      <TabsContent value="scoring" className="flex flex-col gap-4">
        <ScoringRules />
        {evaluation.data ? (
          <Card>
            <CardHeader>
              <CardTitle>Scores in this position</CardTitle>
              <CardDescription>
                {sideName(evaluation.data.side)} · sorted as List 1, fully developed first.
              </CardDescription>
            </CardHeader>
            <CardContent className="flex flex-col">
              {evaluation.data.hits.length === 0 ? (
                <p className="text-sm text-muted-foreground">No strategy meets foundation or progress.</p>
              ) : (
                evaluation.data.hits.map((hit) => (
                  <div key={hit.id} className="flex flex-wrap items-baseline justify-between gap-2 border-b py-2 last:border-b-0">
                    <span className="font-medium">{hit.name}</span>
                    <span className="text-sm text-muted-foreground">
                      {hit.score} · {hit.stageLabel}
                    </span>
                  </div>
                ))
              )}
            </CardContent>
          </Card>
        ) : null}
      </TabsContent>

      <TabsContent value="captures" className="flex flex-col gap-4">
        <CaptureRules />
        {evaluation.data ? <LegalCaptureList evaluation={evaluation.data} /> : null}
      </TabsContent>
    </Tabs>
  );
}

function WindowNote({ fen, moves, side }: { fen: string; moves: RecordedMove[]; side: Color }) {
  const turn = new Chess(fen).turn();
  const window = historyWindow(moves, side, turn);
  const label =
    window.length === 0
      ? "No earlier turns are loaded."
      : window.map((move) => move.san).join(", ");
  const scope = turn === side ? "previous four turns" : "previous four turns plus the current turn";
  return (
    <p className="text-sm leading-relaxed text-muted-foreground">
      <span className="font-medium text-foreground">Turn window. </span>
      {scope}. {label}
    </p>
  );
}

function PositionLists({
  evaluation,
  selectedId,
  onSelect,
}: {
  evaluation: Evaluation;
  selectedId: string | null;
  onSelect: (id: string) => void;
}) {
  const selected = evaluation.hits.find((hit) => hit.id === selectedId) ?? null;
  return (
    <>
      <section className="flex flex-col gap-3" aria-labelledby="list-1-heading">
        <div>
          <h2 id="list-1-heading" className="font-heading text-2xl">
            List 1 · Forming strategies
          </h2>
          <p className="mt-1 text-sm text-muted-foreground">
            {evaluation.hits.length === 0
              ? `${sideName(evaluation.side)} has no strategy at foundation or progress.`
              : `${evaluation.hits.length} strategies, most developed first.`}
          </p>
        </div>
        {evaluation.hits.length === 0 ? (
          <Card>
            <CardContent className="text-sm leading-relaxed">
              Nothing in the turn window or on the board meets a foundation or progress condition.
              A pattern that appears only in a future line is not listed.
            </CardContent>
          </Card>
        ) : (
          <div className="flex flex-col gap-2">
            {evaluation.hits.map((hit) => (
              <button
                key={hit.id}
                type="button"
                onClick={() => onSelect(hit.id)}
                aria-pressed={hit.id === selected?.id}
                className={`rounded-xl border px-3 py-3 text-left ${
                  hit.id === selected?.id ? "border-primary bg-primary/5" : "border-border bg-card hover:bg-muted"
                }`}
              >
                <span className="flex flex-wrap items-center justify-between gap-2">
                  <span className="font-medium">{hit.name}</span>
                  <span className="flex items-center gap-2">
                    <StageBadge stage={hit.stage} label={hit.stageLabel} />
                    <span className="text-sm tabular-nums text-muted-foreground">{hit.score}</span>
                  </span>
                </span>
                <span className="mt-1 block text-sm text-muted-foreground">{hit.evidence}</span>
              </button>
            ))}
          </div>
        )}
      </section>

      {selected ? <StrategyDetail hit={selected} /> : null}

      <section className="flex flex-col gap-2" aria-labelledby="list-2-heading">
        <h2 id="list-2-heading" className="font-heading text-2xl">
          List 2 · Strategy captures
        </h2>
        <p className="text-sm text-muted-foreground">
          {evaluation.sideToMove
            ? "Legal captures by participating pieces, sorted by net value."
            : `It is ${sideName(evaluation.turn)} to move, so ${sideName(evaluation.side)} has no legal capture to price yet.`}
        </p>
        <CaptureStack rows={evaluation.captures} empty="No strategy capture is legal in this position." />
      </section>
    </>
  );
}

function StrategyDetail({ hit }: { hit: StrategyHit }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{hit.name}</CardTitle>
        <CardDescription>{hit.intent}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-3 text-sm leading-relaxed">
        <p>{hit.explanation}</p>
        <dl className="grid gap-2 sm:grid-cols-3">
          <Condition term="Foundation" state={hit.foundation ? "On the board" : "Not met"} />
          <Condition
            term="Progress"
            state={hit.progress ? "On the board" : hit.progressWithinTurn ? "Within one full turn" : "Not met"}
          />
          <Condition
            term="Completion"
            state={hit.completion ? "On the board" : hit.completionWithinTurn ? "Within one full turn" : "Not met"}
          />
        </dl>
        <p>
          <span className="font-medium">Pieces. </span>
          {hit.participants.length === 0
            ? "None."
            : hit.participants.map((unit) => `${pieceName(unit.type)} ${unit.square}`).join(", ")}
        </p>
        <p>
          <span className="font-medium">Targets. </span>
          {hit.targets.length === 0
            ? "None."
            : hit.targets.map((unit) => `${pieceName(unit.type)} ${unit.square}`).join(", ")}
        </p>
      </CardContent>
    </Card>
  );
}

function Condition({ term, state }: { term: string; state: string }) {
  return (
    <div className="rounded-lg bg-muted px-3 py-2">
      <dt className="text-xs tracking-wide text-muted-foreground uppercase">{term}</dt>
      <dd className="mt-1 font-medium">{state}</dd>
    </div>
  );
}

function StageBadge({ stage, label }: { stage: Stage; label: string }) {
  const variant = stage === "full" ? "default" : stage === "partial" ? "secondary" : "outline";
  return <Badge variant={variant}>{label}</Badge>;
}

function CaptureStack({ rows, empty }: { rows: CaptureRow[]; empty: string }) {
  if (rows.length === 0) {
    return <p className="text-sm text-muted-foreground">{empty}</p>;
  }
  return (
    <div className="rounded-xl border bg-card px-3">
      {rows.map((row) => (
        <article
          key={`${row.strategyId}-${row.san}-${row.from}-${row.victimSquare}`}
          className="grid gap-1 border-b py-3 last:border-b-0 sm:grid-cols-[6.5rem_minmax(0,1fr)_8.5rem]"
        >
          <p className="font-heading text-xl">{row.san}</p>
          <div className="text-sm leading-relaxed">
            <p>
              {pieceName(row.capturerType)} on {row.from} takes {pieceName(row.victimType)} on {row.victimSquare}.
            </p>
            <p className="text-muted-foreground">{row.strategyName}</p>
            {row.recaptureSans.length > 0 ? (
              <p className="text-muted-foreground">Recapture {row.recaptureSans.join(", ")}</p>
            ) : (
              <p className="text-muted-foreground">{row.mate ? "Checkmate." : "No recapture."}</p>
            )}
          </div>
          <p className="text-sm sm:text-right">
            <span className="block">Gain {formatMaterial(row.expectedGain)}</span>
            <span className="block">Loss {formatMaterial(row.potentialLoss)}</span>
            <span className="block font-medium">Net {formatSigned(row.net)}</span>
          </p>
        </article>
      ))}
    </div>
  );
}

function FollowUp({
  evaluation,
  followUp,
  nextSan,
  replySan,
  onNext,
  onReply,
}: {
  evaluation: Evaluation;
  followUp: FollowUpResult | null;
  nextSan: string;
  replySan: string;
  onNext: (san: string) => void;
  onReply: (san: string) => void;
}) {
  if (!evaluation.sideToMove) {
    return (
      <Card>
        <CardHeader>
          <CardTitle>Lists 3 and 4</CardTitle>
          <CardDescription>
            Those lists start one move into the future. It is {sideName(evaluation.turn)} to move, so choose{" "}
            {sideName(evaluation.turn)} to step through that side&apos;s next move.
          </CardDescription>
        </CardHeader>
      </Card>
    );
  }

  const replies = followUp?.replies ?? [];
  return (
    <section className="flex flex-col gap-4" aria-labelledby="future-heading">
      <div>
        <h2 id="future-heading" className="font-heading text-2xl">
          One move later
        </h2>
        <p className="mt-1 max-w-3xl text-sm leading-relaxed text-muted-foreground">
          List 3 reads the same 20 strategies after the chosen move. List 4 prices captures after one
          opposing reply, when it is {sideName(evaluation.side)} to move again. Neither step starts a new
          six-move search.
        </p>
      </div>
      <div className="grid gap-3 sm:grid-cols-2">
        <label className="flex flex-col gap-2 text-sm font-medium">
          Next move
          <select
            value={nextSan}
            onChange={(event) => onNext(event.target.value)}
            className="h-9 rounded-lg border border-input bg-card px-2.5 font-normal"
          >
            <option value="">Choose a legal move</option>
            {evaluationLegalMoves(evaluation.fen).map((san) => (
              <option key={san} value={san}>
                {san}
              </option>
            ))}
          </select>
        </label>
        <label className="flex flex-col gap-2 text-sm font-medium">
          Opponent reply
          <select
            value={replySan}
            onChange={(event) => onReply(event.target.value)}
            disabled={replies.length === 0}
            className="h-9 rounded-lg border border-input bg-card px-2.5 font-normal disabled:opacity-60"
          >
            <option value="">{replies.length === 0 ? "Choose a move first" : "Choose a reply"}</option>
            {replies.map((san) => (
              <option key={san} value={san}>
                {san}
              </option>
            ))}
          </select>
        </label>
      </div>
      {followUp?.list3 ? (
        <Card>
          <CardHeader>
            <CardTitle>List 3 · After {nextSan}</CardTitle>
            <CardDescription>
              {followUp.list3.hits.length} forming strategies for {sideName(evaluation.side)}.
            </CardDescription>
          </CardHeader>
          <CardContent className="flex flex-col">
            {followUp.list3.hits.length === 0 ? (
              <p className="text-sm text-muted-foreground">No strategy is forming after this move.</p>
            ) : (
              followUp.list3.hits.map((hit) => (
                <div key={hit.id} className="flex flex-wrap items-baseline justify-between gap-2 border-b py-2 last:border-b-0">
                  <span>
                    <span className="font-medium">{hit.name}</span>
                    <span className="mt-0.5 block text-sm text-muted-foreground">{hit.evidence}</span>
                  </span>
                  <span className="text-sm text-muted-foreground">
                    {hit.score} · {hit.stageLabel}
                  </span>
                </div>
              ))
            )}
          </CardContent>
        </Card>
      ) : null}
      {followUp?.list4 ? (
        <div className="flex flex-col gap-2">
          <h3 className="font-heading text-xl">List 4 · After {replySan}</h3>
          <CaptureStack
            rows={followUp.list4.captures}
            empty="No strategy capture is legal after this reply."
          />
        </div>
      ) : null}
    </section>
  );
}

function ScoringRules() {
  return (
    <div className="grid gap-3 lg:grid-cols-2">
      <Card>
        <CardHeader>
          <CardTitle>One number, then a stage</CardTitle>
          <CardDescription>List 1 and List 3 sort on stage, then on this score.</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-3 text-sm leading-relaxed">
          <p>Completion on the position being scored is fully developed, score 100.</p>
          <p>Foundation and progress, with completion available within one full turn, score 75. That stays partial until the completing position is the one being scored.</p>
          <p>Foundation and progress, with no completion in the next full turn, score 60.</p>
          <p>Foundation now, and progress only within one full turn, scores 45. That is still beginning to form.</p>
          <p>Foundation alone, or progress alone, scores 30.</p>
          <p>A future line never adds a strategy by itself. The row exists only when foundation or progress is already true, or completion is already true.</p>
        </CardContent>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle>What “one full turn” means</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-3 text-sm leading-relaxed">
          <p>One full turn is two plies: the side&apos;s move and the opponent&apos;s reply, or the reverse when the opponent is already to move. Every legal move at both plies is examined.</p>
          <p>Progress or completion counts as “within one full turn” if it is true after the side&apos;s move or still true after the reply. The search stops at checkmate.</p>
          <p>The turn window is the previous four turns by that side. When that side has just moved, the current turn is included as a fifth move. The central pawn break is complete only when its pawn capture is inside this window.</p>
          <p>Attacks used for recognition are geometric. A pinned or blocked piece still counts. Capture valuation uses legal moves only.</p>
        </CardContent>
      </Card>
    </div>
  );
}

function CaptureRules() {
  const rows = [
    ["Pawn", PIECE_VALUE.p],
    ["Knight", PIECE_VALUE.n],
    ["Bishop", PIECE_VALUE.b],
    ["Rook", PIECE_VALUE.r],
    ["Queen", PIECE_VALUE.q],
  ] as const;
  return (
    <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_16rem]">
      <Card>
        <CardHeader>
          <CardTitle>Expected gain minus potential loss</CardTitle>
          <CardDescription>The capture itself is always played. Recaptures may stop.</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-3 text-sm leading-relaxed">
          <p>
            Expected gain is the total value of opposing pieces the original player takes in the priced
            sequence. Potential loss is the total value of that player&apos;s pieces the opponent takes back.
            Net value is gain minus loss. List 2 and List 4 sort by net, then by higher gain, then by lower loss.
          </p>
          <p>
            After the capture, only the cheapest piece that can legally recapture on that square is considered.
            If several pieces share that value, each is tried and the side to move keeps the result it prefers.
            A promoting pawn is still priced as a pawn when choosing the cheapest piece. The promotion itself adds
            the new piece&apos;s value minus a pawn.
          </p>
          <p>
            Each side stops when the next recapture does not improve its result. An equal net stops rather than
            trading more pieces. Checkmate adds 100000 to the mating side&apos;s column: expected gain if the
            original player mates, potential loss if the opponent&apos;s recapture mates. A king has no capture
            value. En passant removes the pawn from its own square and prices recaptures on the landing square.
          </p>
          <p>
            A capture belongs to a strategy only when the moving piece is one of that strategy&apos;s
            participating pieces and the captured piece is one of its targets. The same move can appear once
            for each strategy that claims it.
          </p>
        </CardContent>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle>Piece values</CardTitle>
        </CardHeader>
        <CardContent>
          <dl className="flex flex-col">
            {rows.map(([name, value]) => (
              <div key={name} className="flex justify-between border-b py-1.5 text-sm last:border-b-0">
                <dt>{name}</dt>
                <dd className="tabular-nums">{value}</dd>
              </div>
            ))}
          </dl>
        </CardContent>
      </Card>
    </div>
  );
}

function LegalCaptureList({ evaluation }: { evaluation: Evaluation }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Every legal capture</CardTitle>
        <CardDescription>
          {evaluation.sideToMove
            ? "Priced with the same rule, whether or not a strategy claims the move."
            : `${sideName(evaluation.side)} is not to move, so there is no legal capture to price.`}
        </CardDescription>
      </CardHeader>
      <CardContent>
        {evaluation.legalCaptures.length === 0 ? (
          <p className="text-sm text-muted-foreground">No legal capture for this side.</p>
        ) : (
          <div className="flex flex-col">
            {evaluation.legalCaptures.map((row) => (
              <LegalRow key={`${row.san}-${row.from}-${row.victimSquare}`} row={row} />
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

function LegalRow({ row }: { row: ValuedCapture }) {
  return (
    <article className="grid gap-1 border-b py-3 last:border-b-0 sm:grid-cols-[6.5rem_minmax(0,1fr)_8.5rem]">
      <p className="font-heading text-xl">{row.san}</p>
      <div className="text-sm leading-relaxed">
        <p>
          {pieceName(row.capturerType)} on {row.from} takes {pieceName(row.victimType)} on {row.victimSquare}.
        </p>
        <p className="text-muted-foreground">
          {row.strategies.length > 0 ? row.strategies.map((strategy) => strategy.name).join(", ") : "Not claimed by a forming strategy."}
        </p>
      </div>
      <p className="text-sm sm:text-right">
        <span className="block">Gain {formatMaterial(row.valuation.expectedGain)}</span>
        <span className="block">Loss {formatMaterial(row.valuation.potentialLoss)}</span>
        <span className="block font-medium">Net {formatSigned(row.valuation.net)}</span>
      </p>
    </article>
  );
}

interface FollowUpResult {
  replies: string[];
  list3: Evaluation;
  list4: Evaluation | null;
}

function evaluationLegalMoves(fen: string): string[] {
  return new Chess(fen).moves();
}

function buildFollowUp(
  fen: string,
  side: Color,
  moves: RecordedMove[],
  nextSan: string,
  replySan: string,
): FollowUpResult | null {
  if (!nextSan) return null;
  try {
    const chess = new Chess(fen);
    if (chess.turn() !== side) return null;
    const played = chess.move(nextSan);
    const nextMoves = [...moves, recordMove(played)];
    const list3 = evaluatePosition(chess, side, nextMoves);
    const replies = chess.moves();
    let list4: Evaluation | null = null;
    if (replySan && replies.includes(replySan)) {
      const branch = new Chess(chess.fen());
      const reply = branch.move(replySan);
      list4 = evaluatePosition(branch, side, [...nextMoves, recordMove(reply)]);
    }
    return { replies, list3, list4 };
  } catch {
    return null;
  }
}
