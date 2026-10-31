/** Words for the decks page: format names, the check state and its note, card counts and the kind of a pasted list. */
import { BY_HAND, BY_HAND_TITLE, byHandCopies, cardsByHand } from '@/lib/byHand'
import type {
  Counts,
  DeckListItem,
  DeckState,
  DeckZone,
  ImportResult,
  ImportSource,
  Support,
} from '@/protocol/generated'

// In the order of `SUPPORTED_FORMATS` in `crates/mtg-cards/src/deck/legality.rs`.
const FORMAT_LABEL: Readonly<Record<string, string>> = {
  commander: 'Commander',
  freeform: 'Freeform',
  standard: 'Standard',
  pioneer: 'Pioneer',
  modern: 'Modern',
  legacy: 'Legacy',
  vintage: 'Vintage',
  pauper: 'Pauper',
  historic: 'Historic',
  timeless: 'Timeless',
  alchemy: 'Alchemy',
  penny: 'Penny Dreadful',
  premodern: 'Premodern',
  oldschool: 'Old School',
}

/** The format keys a deck can be saved in, in the order the pickers list them. */
export const DECK_FORMATS: readonly string[] = Object.keys(FORMAT_LABEL)

/** The format a new deck starts with. */
export const DEFAULT_FORMAT = 'commander'

/** The name of a format key; a key this page does not know is shown as it is. */
export function formatLabel(key: string): string {
  return (Object.hasOwn(FORMAT_LABEL, key) ? FORMAT_LABEL[key] : undefined) ?? key
}

/** The word of each check state; the badge shows it beside an icon so colour never carries the meaning alone. */
export const STATE_LABEL: Readonly<Record<DeckState, string>> = {
  verified: 'Verified',
  manual_play: BY_HAND_TITLE,
  blocked: 'Blocked',
  unchecked: 'Unchecked',
}

/** What a blocked deck cannot do, in the lobby's words, for the badge's hover and the pages that say more. */
export const BLOCKED_MEANING = 'Cannot be taken to a game until its issues are fixed'

/** The word of each support state; a mark shows it beside an icon so colour never carries the meaning alone. */
export const SUPPORT_LABEL: Readonly<Record<Support, string>> = {
  verified: 'Verified',
  read: 'Read',
  partial: 'Partial',
  manual: 'Manual',
  stale: 'Stale',
  unplayable: 'Unplayable',
}

/** "1 card", "100 cards". */
export const cardsText = (count: number): string => `${count} ${count === 1 ? 'card' : 'cards'}`

const plural = (count: number, noun: string): string => `${count} ${noun}${count === 1 ? '' : 's'}`

/**
 * What a check found, in copies: "96 of 100 verified, 4 played by hand, 1 unplayable". Only the states that have copies are named.
 */
export function countsText(counts: Counts): string {
  const hand = byHandCopies(counts)
  const parts = [
    counts.read > 0 ? `${counts.read} on reads` : null,
    hand > 0 ? `${hand} ${BY_HAND}` : null,
    counts.unplayable > 0 ? `${counts.unplayable} unplayable` : null,
  ].filter((part) => part !== null)
  return [`${counts.verified} of ${counts.total} verified`, ...parts].join(', ')
}

/** True when a check found a card the game does not run whole (partial, manual, stale or unplayable). */
export const hasUnvalidated = (counts: Counts): boolean =>
  counts.partial + counts.manual + counts.stale + counts.unplayable > 0

/**
 * The words of the link to the cards a check found the game does not run whole: "Show the 7 cards played by hand",
 * "Show the 2 unplayable cards", or both. Empty when it found none.
 */
export function unvalidatedLink(counts: Counts): string {
  const hand = byHandCopies(counts)
  const parts = [
    hand > 0 ? cardsByHand(hand) : null,
    counts.unplayable > 0 ? plural(counts.unplayable, 'unplayable card') : null,
  ].filter((part) => part !== null)
  return parts.length === 0 ? '' : `Show the ${parts.join(' and ')}`
}

/**
 * The line under a deck's name that explains its check state: how many cards are played by hand, what blocks it, or that
 * it was never checked. A verified deck, and one that changed since its check, need no line: its marker says so.
 */
export function checkNote(deck: DeckListItem): string | null {
  const { counts, issues } = deck.summary
  switch (deck.state) {
    case 'verified':
      return null
    case 'manual_play':
      return countsText(counts)
    case 'blocked': {
      const parts = [
        issues > 0 ? plural(issues, 'legality issue') : null,
        counts.unplayable > 0 ? plural(counts.unplayable, 'unplayable card') : null,
      ].filter((part) => part !== null)
      return `${parts.length > 0 ? `${parts.join(', ')}. ` : ''}${BLOCKED_MEANING}.`
    }
    case 'unchecked':
      return deck.changed ? null : 'Not checked yet'
  }
}

/** The kind of list the server says it read: "Arena", "MTG Online". */
export const SOURCE_LABEL: Readonly<Record<ImportSource, string>> = {
  text: 'plain text',
  arena: 'Arena',
  mtgo: 'MTG Online',
  mtgo_dek: 'MTG Online (.dek)',
  moxfield: 'Moxfield',
  archidekt: 'Archidekt',
}

// The zones in the order a deck is read: the main deck first, then what sits beside it.
const ZONE_PHRASE: Readonly<Record<DeckZone, (count: number) => string>> = {
  main: (count) => `${count} in the main deck`,
  sideboard: (count) => `${count} in the sideboard`,
  commander: (count) => plural(count, 'commander'),
  companion: (count) => plural(count, 'companion'),
}

/** "Read as Arena: 100 cards (99 in the main deck, 1 commander)." for what an import resolved. */
export function importSummary(result: ImportResult): string {
  const byZone = new Map<DeckZone, number>()
  for (const line of result.cards) byZone.set(line.zone, (byZone.get(line.zone) ?? 0) + line.count)
  const total = [...byZone.values()].reduce((sum, count) => sum + count, 0)
  const zones = (Object.keys(ZONE_PHRASE) as DeckZone[]).flatMap((zone) => {
    const count = byZone.get(zone)
    return count === undefined ? [] : [ZONE_PHRASE[zone](count)]
  })
  const detail = zones.length > 1 ? ` (${zones.join(', ')})` : ''
  return `Read as ${SOURCE_LABEL[result.source]}: ${cardsText(total)}${detail}.`
}
