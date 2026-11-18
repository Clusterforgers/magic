/**
 * What this seat answered to the triggers of the run it is in, so the column's done rows say the outcome ("Yes · Draw a
 * card"). One run at a time: an answer for another run forgets the last.
 */
let run: number | null = null
let outcomes = new Map<number, string>()

/** Remembers `words` as the outcome of the trigger at `place` (from 1) of run `id`. */
export function rememberOutcome(id: number, place: number, words: string): void {
  if (run !== id) {
    run = id
    outcomes = new Map()
  }
  outcomes.set(place, words)
}

/** The outcome remembered for the trigger at `place` (from 1) of run `id`. */
export function outcomeOf(id: number, place: number): string | undefined {
  return run === id ? outcomes.get(place) : undefined
}

/** Forgets every outcome (tests). */
export function forgetOutcomes(): void {
  run = null
  outcomes = new Map()
}
