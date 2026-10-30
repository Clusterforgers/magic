/**
 * The decks the chooser lists as one kind of row, whatever they come from (stored deck, precon, built-in deck), and
 * the search and filters over them. Pure, so the rows' words and the filters are tested without a page.
 */
import { BY_HAND, byHandCopies } from '@/lib/byHand'
import { cardsText } from '@/features/decks/format'
import type { DeckInfo } from '@/net/api'
import type { Counts, DeckListItem, DeckState, PreconListItem } from '@/protocol/generated'
import { deckFormat } from './deckWords'

/** Where a deck comes from: the account, the server's precons or the built-in decks. */
export type DeckKind = 'own' | 'precon' | 'builtin'

/** One deck of the chooser's list. */
export interface DeckEntry {
  /** The id the server takes: a stored deck's, `precon:<code>` or a built-in deck's. */
  id: string
  kind: DeckKind
  name: string
  /** The format key, `normal` for a built-in deck. */
  format: string
  /** Copies the game plays. */
  cards: number
  /** The commanders the list knows a picture of. */
  commanders: { oracleId: string; name: string }[]
  /** The commanders' names, also where the list holds no picture. */
  commanderNames: string[]
  /** Colour identity letters in WUBRG order; null where the list does not say. */
  identity: string[] | null
  /** Copies by support state; null for a built-in deck, which the game runs whole. */
  counts: Counts | null
  /** The deck's own check state. */
  state: DeckState
  /** The set code of a precon. */
  set: string | null
  /** What else search matches: the kind of precon. */
  extra: string
  /** The cards by name for a built-in deck, whose list holds no card data; empty for the others. */
  listed: { name: string; count: number }[]
}

/** What the list can be narrowed to by support. */
export type SupportFilter = 'all' | 'verified' | 'hand'

/** The narrowing of a list: search words, colours the deck's identity holds, support. */
export interface DeckFilter {
  query: string
  colours: readonly string[]
  support: SupportFilter
}

/** No narrowing. */
export const NO_FILTER: DeckFilter = { query: '', colours: [], support: 'all' }

/** Whether a filter narrows the list at all. */
export const isFiltered = (filter: DeckFilter): boolean =>
  filter.query.trim() !== '' || filter.colours.length > 0 || filter.support !== 'all'

const WUBRG = ['W', 'U', 'B', 'R', 'G']

/** The commanders' identity letters together in WUBRG order. */
function identityOf(letters: readonly (readonly string[])[]): string[] {
  const all = new Set(letters.flat())
  return WUBRG.filter((letter) => all.has(letter))
}

/** A stored deck as a row; the commanders' pictures and identity come with the list. */
export function entryOfDeck(deck: DeckListItem): DeckEntry {
  return {
    id: deck.id,
    kind: 'own',
    name: deck.name,
    format: deck.format,
    cards: deck.summary.counts.total,
    commanders: deck.commanders.map((card) => ({ oracleId: card.oracle_id, name: card.name })),
    commanderNames: deck.commanders.map((card) => card.name),
    identity:
      deck.commanders.length === 0 ? null : identityOf(deck.commanders.map((card) => card.color_identity)),
    counts: deck.summary.counts,
    state: deck.state,
    set: null,
    extra: '',
    listed: [],
  }
}

/** A precon as a row; its list item names the commanders but carries neither their pictures nor their colours. */
export function entryOfPrecon(precon: PreconListItem): DeckEntry {
  return {
    id: `precon:${precon.code}`,
    kind: 'precon',
    name: precon.name,
    format: precon.format,
    cards: precon.card_count,
    commanders: [],
    commanderNames: precon.commanders,
    identity: null,
    counts: precon.summary.counts,
    state: precon.playable ? precon.state : 'blocked',
    set: precon.set,
    extra: precon.deck_type ?? '',
    listed: [],
  }
}

/** A built-in deck as a row: a Normal deck the whole game runs. */
export function entryOfBuiltIn(deck: DeckInfo): DeckEntry {
  return {
    id: deck.id,
    kind: 'builtin',
    name: deck.name,
    format: 'normal',
    cards: deck.size,
    commanders: [],
    commanderNames: [],
    identity: null,
    counts: null,
    state: 'verified',
    set: null,
    extra: deck.color,
    listed: deck.cards,
  }
}

/** The facts under a row's name: "Commander, 100 cards", with the set of a precon first. */
export function entryFacts(entry: DeckEntry): string {
  return [entry.set?.toUpperCase() ?? null, deckFormat(entry.format), cardsText(entry.cards)]
    .filter((part) => part !== null)
    .join(' · ')
}

/** The chip of a row: "Whole deck runs", "9 played by hand", "Cannot be taken" or "Not checked yet". */
export function entrySupport(entry: DeckEntry): { state: DeckState; label: string } {
  if (entry.state === 'blocked') return { state: 'blocked', label: 'Cannot be taken' }
  if (entry.state === 'unchecked') return { state: 'unchecked', label: 'Not checked yet' }
  const hand = entry.counts === null ? 0 : byHandCopies(entry.counts)
  if (entry.state === 'verified' && hand === 0) return { state: 'verified', label: 'Whole deck runs' }
  return { state: 'manual_play', label: `${hand} ${BY_HAND}` }
}

const words = (text: string): string[] =>
  text
    .toLowerCase()
    .split(/\s+/)
    .filter((word) => word !== '')

/** The entries that match every search word (in name, set, commander or kind), hold every chosen colour and the support. */
export function filterEntries(entries: readonly DeckEntry[], filter: DeckFilter): DeckEntry[] {
  const wanted = words(filter.query)
  return entries.filter((entry) => {
    const haystack = [entry.name, entry.set ?? '', entry.extra, ...entry.commanderNames]
      .join(' ')
      .toLowerCase()
    if (!wanted.every((word) => haystack.includes(word))) return false
    if (filter.colours.length > 0) {
      const identity = entry.identity
      if (identity === null || !filter.colours.every((letter) => identity.includes(letter))) return false
    }
    if (filter.support === 'all') return true
    const hand = entry.counts === null ? 0 : byHandCopies(entry.counts)
    if (filter.support === 'verified') return entry.state === 'verified' && hand === 0
    return hand > 0
  })
}

/** Whether a deck of this format may be taken to a game of `format`: a Commander game takes decks with a commander. */
export const fitsFormat = (entry: Pick<DeckEntry, 'format'>, commanderGame: boolean): boolean =>
  commanderGame === (entry.format === 'commander')

/** The entry that marks the seat's deck: the one the last answer named, else the one deck with that name and format. */
export function markedId(
  entries: readonly Pick<DeckEntry, 'id' | 'name' | 'format'>[],
  current: { name: string; format: string } | null | undefined,
  chosenId: string | null,
): string | null {
  if (chosenId !== null && entries.some((entry) => entry.id === chosenId)) return chosenId
  if (current === null || current === undefined) return null
  const same = entries.filter((entry) => entry.name === current.name && entry.format === current.format)
  return same.length === 1 ? (same[0]?.id ?? null) : null
}
