import { Studio } from "@/components/studio";

export default function HomePage() {
  return (
    <main className="mx-auto flex w-full max-w-6xl flex-col gap-8 px-4 py-8 sm:px-6 sm:py-10">
      <header className="max-w-3xl">
        <p className="text-sm font-medium tracking-wide text-primary uppercase">Reference definitions</p>
        <h1 className="mt-2 font-heading text-4xl leading-tight text-balance sm:text-5xl">
          Forming strategies
        </h1>
        <p className="mt-3 text-base leading-relaxed text-muted-foreground sm:text-lg">
          The program already knew how to list legal moves, keep four turns of history, and search one
          full turn ahead. It still needed three definitions: the 20 strategies, how developed each one
          is, and the gain and loss of every capture.
        </p>
      </header>
      <Studio />
    </main>
  );
}
