/** The one wording for cards the engine plays only in part, so that the lobby, the choosers and the builder agree. */
import type { Counts } from '@/protocol/generated'

/** What is said of a card, or a rule of one, that the players resolve themselves: "3 cards played by hand". */
export const BY_HAND = 'played by hand'

/** `BY_HAND` at the start of a sentence, a title or a label. */
export const BY_HAND_TITLE = 'Played by hand'

/** A card of which the engine runs one clause and leaves another to the players. */
export const PARTLY_BY_HAND = 'partly played by hand'

/** Copies the game does not run whole (everything but Verified and Unplayable): the partial, manual and stale ones. */
export const byHandCopies = (counts: Counts): number => counts.partial + counts.manual + counts.stale

/** "no cards played by hand", "1 card played by hand", "3 cards played by hand". */
export function cardsByHand(count: number): string {
  if (count === 0) return `no cards ${BY_HAND}`
  return `${count} ${count === 1 ? 'card' : 'cards'} ${BY_HAND}`
}
