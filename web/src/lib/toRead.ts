/** The one wording and address for cards that wait for a player's read, so every deck check agrees, and the words of the tiers that run before a read (docs/web.md, "Accounts and games pages"). */
import type { Counts } from '@/protocol/generated'
import { BY_HAND } from './byHand'

/** What is said of a draft nobody has judged, in a count: "3 cards to read". */
export const TO_READ = 'to read'

/** The name of a filter or a heading for the cards that wait for a read. */
export const TO_READ_TITLE = 'To read'

/** The deck state of a deck the game runs whole, whether or not every card of it is Verified. */
export const WHOLE_DECK_RUNS = 'Whole deck runs'

/** The word of a card whose draft runs before any player read it. */
export const UNVERIFIED = 'Unverified'

/** What an unverified card is, in plain words: the game runs it, nobody has read it yet. */
export const UNVERIFIED_MEANING = 'Runs automatically, not verified: nobody has read its rules yet'

/** The link that opens the read of an unverified card. */
export const READ_IT = 'Read it'

/** What a card translated from a Forge script is, in plain words: the game runs it, nobody verified it. */
export const FROM_FORGE = 'Runs automatically, translated from Forge, not verified'

/** "1 card from Forge", "4 cards from Forge": copies of cards that run on a Forge translation. */
export function cardsFromForge(count: number): string {
  return `${count} ${count === 1 ? 'card' : 'cards'} from Forge`
}

/** "no cards to read", "1 card to read", "12 cards to read". */
export function cardsToRead(count: number): string {
  if (count === 0) return `no cards ${TO_READ}`
  return `${count} ${count === 1 ? 'card' : 'cards'} ${TO_READ}`
}

/**
 * The quiet line under a deck the game runs whole: its copies that are not Verified, by tier, "34 from Forge · 120
 * unverified · 2 read"; empty when every copy is Verified.
 */
export function runsBreakdown(counts: Pick<Counts, 'forge' | 'unverified' | 'read'>): string {
  return [
    counts.forge > 0 ? `${counts.forge} from Forge` : null,
    counts.unverified > 0 ? `${counts.unverified} unverified` : null,
    counts.read > 0 ? `${counts.read} read` : null,
  ]
    .filter((part) => part !== null)
    .join(' · ')
}

/** What a draft that is on a card of a deck says of its read: nobody read it yet, or a player found it wrong. */
export const DRAFT_NOTE: Readonly<Record<'pending' | 'wrong', string>> = {
  pending: 'A draft of its rules waits for a player to read it',
  wrong: `A player found the draft of its rules wrong, so the card stays ${BY_HAND}`,
}

/** The cards to be read: a stored deck by id, a precon by code, or those of the account's own decks. */
export type ReadTarget = { deck: string } | { precon: string } | { mine: true }

/** The address of the review screen for `target`: `/read?deck=<id>`, `/read?precon=<code>` or `/read?mine=true`. */
export function readPath(target: ReadTarget): string {
  if ('mine' in target) return '/read?mine=true'
  return 'deck' in target
    ? `/read?deck=${encodeURIComponent(target.deck)}`
    : `/read?precon=${encodeURIComponent(target.precon)}`
}
