/** The words of the response clock: what the server does when it runs out, and how long is left. */
import type { DecisionName } from '../turnText'

/** What the server answers for a seat whose clock ran out, as the countdown puts it. Only these two kinds have a clock. */
const OUTCOME: Partial<Record<DecisionName, string>> = {
  Priority: 'Passing for you',
  DeclareBlockers: 'Declaring no blocks',
}

/** The countdown in plain words: "Passing for you in 7 s". */
export function clockText(kind: DecisionName, seconds: number): string {
  return `${OUTCOME[kind] ?? 'The game goes on without you'} in ${seconds} s`
}

/** What a screen reader hears once when the clock starts: "You have 10 seconds; after that the game passes for you." */
export function clockAnnouncement(kind: DecisionName, seconds: number): string {
  const unit = seconds === 1 ? 'second' : 'seconds'
  const then = kind === 'DeclareBlockers' ? 'declares no blocks for you' : 'passes for you'
  return `You have ${seconds} ${unit} to answer; after that the game ${then}.`
}

/** Whole seconds shown for `leftMs`: the number counts down to 1 while any time is left. */
export function secondsLeft(leftMs: number): number {
  return Math.max(0, Math.ceil(leftMs / 1000))
}

/** The last seconds, when the countdown is drawn as urgent (it also says so in words, never by colour alone). */
export const URGENT_SECONDS = 3
