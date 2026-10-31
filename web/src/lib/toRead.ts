/** The one wording and address for cards that wait for a player's read, so every deck check agrees (docs/web.md, "Reading cards"). */
import { BY_HAND } from './byHand'

/** What is said of a draft nobody has judged, in a count: "3 cards to read". */
export const TO_READ = 'to read'

/** The name of a filter or a heading for the cards that wait for a read. */
export const TO_READ_TITLE = 'To read'

/** The deck state of a deck whose only cards that are not Verified are approved reads. */
export const RUNS_ON_READS = 'Runs on reads'

/** "no cards to read", "1 card to read", "12 cards to read". */
export function cardsToRead(count: number): string {
  if (count === 0) return `no cards ${TO_READ}`
  return `${count} ${count === 1 ? 'card' : 'cards'} ${TO_READ}`
}

/** "1 card on reads", "4 cards on reads": copies of cards that run on a player's read. */
export function cardsOnReads(count: number): string {
  return `${count} ${count === 1 ? 'card' : 'cards'} on reads`
}

/** What a draft that is on a card of a deck says of its read: nobody read it yet, or a player found it wrong. */
export const DRAFT_NOTE: Readonly<Record<'pending' | 'wrong', string>> = {
  pending: 'A draft of its rules waits for a player to read it',
  wrong: `A player found the draft of its rules wrong, so the card stays ${BY_HAND}`,
}

/** The deck whose cards are to be read: a stored deck by id or a precon by code. */
export type ReadTarget = { deck: string } | { precon: string }

/** The address of the review screen for `target`: `/read?deck=<id>` or `/read?precon=<code>`. */
export function readPath(target: ReadTarget): string {
  return 'deck' in target
    ? `/read?deck=${encodeURIComponent(target.deck)}`
    : `/read?precon=${encodeURIComponent(target.precon)}`
}
