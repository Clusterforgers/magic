/** The words of the response clock: what the server does when it runs out, and how long is left. */
import type { DecisionName } from '../turnText'

/** What the server answers for a seat whose clock ran out, by kind: as the countdown puts it and as it is announced. */
const AT_ZERO: Partial<Record<DecisionName, { countdown: string; announced: string }>> = {
  Priority: { countdown: 'Passing for you', announced: 'passes for you' },
  DeclareBlockers: { countdown: 'Declaring no blocks', announced: 'declares no blocks for you' },
  ChooseTargets: { countdown: 'Choosing no targets', announced: 'chooses no targets for you' },
  OrderTriggers: { countdown: 'Ordering the triggers for you', announced: 'orders the triggers for you' },
}

/** The countdown in plain words: "Passing for you in 7 s". */
export function clockText(kind: DecisionName, seconds: number): string {
  return `${AT_ZERO[kind]?.countdown ?? 'The game goes on without you'} in ${seconds} s`
}

/** What a screen reader hears once when the clock starts: "You have 10 seconds; after that the game passes for you." */
export function clockAnnouncement(kind: DecisionName, seconds: number): string {
  const unit = seconds === 1 ? 'second' : 'seconds'
  return `You have ${seconds} ${unit} to answer; after that the game ${AT_ZERO[kind]?.announced ?? 'passes for you'}.`
}

/** Whole seconds shown for `leftMs`: the number counts down to 1 while any time is left. */
export function secondsLeft(leftMs: number): number {
  return Math.max(0, Math.ceil(leftMs / 1000))
}

/** The last seconds, when the countdown is drawn as urgent (it also says so in words, never by colour alone). */
export const URGENT_SECONDS = 3
