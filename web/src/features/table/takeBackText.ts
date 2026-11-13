/** Take-backs in words: what one undoes, how a request ended, and why one was refused. */
import type { TakeBackKind, TakeBackOffer, TakeBackOutcome, TakeBackRefusal } from '@/protocol/generated'
import type { TakeBackRequest } from '@/play/store'
import type { SeatNamer } from './names'

/** How long the notice of an ended or refused take-back stays before it goes by itself. */
export const TAKE_BACK_NOTICE_MS = 6000

/** Whole seconds left of `request`'s clock at `now`, never below 0; a `now` from before the request counts as its start. */
export function secondsLeft(request: TakeBackRequest, now: number): number {
  return Math.max(0, Math.ceil((request.deadlineMs - Math.max(0, now - request.at)) / 1000))
}

const VERBS: Record<TakeBackKind, string> = { Cast: 'casting', Activation: 'activating', Land: 'playing' }

/** The action a take-back undoes: "casting Shock", "activating Prodigal Sorcerer", "playing Forest". */
export function takeBackAction(kind: TakeBackKind, name: string): string {
  return `${VERBS[kind]} ${name}`
}

/** Why the seat's take-back was refused, as one sentence. */
export const REFUSAL_WORDS: Record<TakeBackRefusal, string> = {
  Off: 'Take-backs are off in this game.',
  Nothing: 'There is nothing to take back.',
  MovedOn: 'Too late to take that back: the game has moved on.',
  OtherPlayerActed: 'Too late to take that back: another player has acted since.',
  CardDrawn: 'Too late to take that back: a card was drawn since.',
  CardRevealed: 'Too late to take that back: a card was revealed since.',
  LibrarySearched: 'Too late to take that back: a library was searched since.',
  LibraryShuffled: 'Too late to take that back: a library was shuffled since.',
  Random: 'Too late to take that back: something random happened since.',
}

/** How a request to take back `offer` ended, as one sentence. */
export function outcomeWords(offer: TakeBackOffer, outcome: TakeBackOutcome, seat: SeatNamer): string {
  const action = takeBackAction(offer.kind, offer.name)
  switch (outcome.type) {
    case 'Approved':
      return `Approved: ${seat(offer.player)} takes back ${action}.`
    case 'Declined':
      return `Declined by ${seat(outcome.by)}: ${action} stays.`
    case 'TimedOut':
      return `No answer from ${seat(outcome.by)}: ${action} stays.`
    case 'Withdrawn':
      return `Withdrawn: the request to take back ${action} no longer applies.`
  }
}
