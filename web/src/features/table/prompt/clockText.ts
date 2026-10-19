/** The words of the response clock: what the server does when it runs out, and how long is left. */
import type { DecisionKind } from '@/protocol/generated'
import { decisionName } from '../turnText'
import type { DecisionName } from '../turnText'

/** What a clock counts down for: a decision kind, or a kind whose answer at zero depends on its options. */
export type ClockKind = DecisionName | 'ChooseModeCancel' | 'ChooseCardsNone'

/**
 * The clock kind of a decision: a choice of modes that can be backed out of is told apart, since the server takes the
 * cast back for it, and so is a choice of cards that may be none, since the server then chooses none.
 */
export function clockKind(kind: DecisionKind): ClockKind {
  if ('ChooseMode' in kind && kind.ChooseMode.can_cancel) return 'ChooseModeCancel'
  if ('ChooseCards' in kind && kind.ChooseCards.min === 0) return 'ChooseCardsNone'
  return decisionName(kind)
}

/** What the server answers for a seat whose clock ran out, by kind: as the countdown puts it and as it is announced. */
const AT_ZERO: Partial<Record<ClockKind, { countdown: string; announced: string }>> = {
  Priority: { countdown: 'Passing for you', announced: 'passes for you' },
  DeclareBlockers: { countdown: 'Declaring no blocks', announced: 'declares no blocks for you' },
  ChooseTargets: { countdown: 'Choosing no targets', announced: 'chooses no targets for you' },
  OrderTriggers: { countdown: 'Ordering the triggers for you', announced: 'orders the triggers for you' },
  YesNo: { countdown: 'Answering no for you', announced: 'answers no for you' },
  ChooseOption: {
    countdown: 'Choosing the first option for you',
    announced: 'chooses the first option for you',
  },
  ChooseMode: { countdown: 'Choosing the first mode for you', announced: 'chooses the first mode for you' },
  ChooseModeCancel: { countdown: 'Cancelling for you', announced: 'cancels for you' },
  ChooseCards: {
    countdown: 'Choosing the first cards for you',
    announced: 'chooses the first cards for you',
  },
  ChooseCardsNone: { countdown: 'Choosing no cards for you', announced: 'chooses no cards for you' },
  OrderCards: { countdown: 'Keeping the order for you', announced: 'keeps the order for you' },
}

/** The countdown in plain words: "Passing for you in 7 s". */
export function clockText(kind: ClockKind, seconds: number): string {
  return `${AT_ZERO[kind]?.countdown ?? 'The game goes on without you'} in ${seconds} s`
}

/** What a screen reader hears once when the clock starts: "You have 10 seconds; after that the game passes for you." */
export function clockAnnouncement(kind: ClockKind, seconds: number): string {
  const unit = seconds === 1 ? 'second' : 'seconds'
  return `You have ${seconds} ${unit} to answer; after that the game ${AT_ZERO[kind]?.announced ?? 'passes for you'}.`
}

/** Whole seconds shown for `leftMs`: the number counts down to 1 while any time is left. */
export function secondsLeft(leftMs: number): number {
  return Math.max(0, Math.ceil(leftMs / 1000))
}

/** The last seconds, when the countdown is drawn as urgent (it also says so in words, never by colour alone). */
export const URGENT_SECONDS = 3
