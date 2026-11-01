/**
 * The decks the chooser lists as one kind of row, whatever they come from (stored deck, precon, built-in deck), and
 * the search and filters over them. Pure, so the rows' words and the filters are tested without a page.
 */
import { BY_HAND, byHandCopies, cardsByHand } from '@/lib/byHand'
import { RUNS_ON_READS } from '@/lib/toRead'
import { cardsText } from '@/features/decks/format'
import type { DeckInfo } from '@/net/api'
import type { Counts, DeckListItem, DeckPicture, DeckState, PreconListItem } from '@/protocol/generated'
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
  /** The cards whose pictures stand for the deck in the list: its commanders, else its first card; none where the list knows no card. */
  pictures: { oracleId: string; name: string }[]
  /** The commanders' names, also where the list holds no picture. */
  commanderNames: string[]
  /** Colour identity letters in WUBRG order; null where the list does not say. */
  identity: string[] | null
  /** Copies by support state; null for a built-in deck, which the game runs whole. */
  counts: Counts | null
  /** Copies the game leaves to the players (partial, manual and stale): what "fewest played by hand" orders by. */
  byHand: number
  /** A precon the account starred. */
  favourite: boolean
  /** The deck's own check state. */
  state: DeckState
  /** Distinct cards whose draft waits for a read; none for a built-in deck. */
  toRead: number
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

const pictureOf = (picture: DeckPicture): { oracleId: string; name: string } => ({
  oracleId: picture.oracle_id,
  name: picture.name,
})

/** A stored deck as a row; the commanders' pictures and identity come with the list. */
export function entryOfDeck(deck: DeckListItem): DeckEntry {
  return {
    id: deck.id,
    kind: 'own',
    name: deck.name,
    format: deck.format,
    cards: deck.summary.counts.total,
    commanders: deck.commanders.map((card) => ({ oracleId: card.oracle_id, name: card.name })),
    pictures: deck.pictures.map(pictureOf),
    commanderNames: deck.commanders.map((card) => card.name),
    identity:
      deck.commanders.length === 0 ? null : identityOf(deck.commanders.map((card) => card.color_identity)),
    counts: deck.summary.counts,
    byHand: byHandCopies(deck.summary.counts),
    favourite: false,
    state: deck.state,
    toRead: deck.summary.to_read,
    set: null,
    extra: '',
    listed: [],
  }
}

/** A precon as a row; its list item names the commanders but carries neither their pictures nor their colours. */
export function entryOfPrecon(precon: PreconListItem, favourite = false): DeckEntry {
  return {
    id: `precon:${precon.code}`,
    kind: 'precon',
    name: precon.name,
    format: precon.format,
    cards: precon.card_count,
    commanders: [],
    commanderNames: precon.commanders,
    pictures: precon.pictures.map(pictureOf),
    identity: null,
    counts: precon.summary.counts,
    byHand: byHandCopies(precon.summary.counts),
    favourite,
    state: precon.playable ? precon.state : 'blocked',
    toRead: precon.summary.to_read,
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
    pictures: [],
    identity: null,
    counts: null,
    byHand: 0,
    favourite: false,
    state: 'verified',
    toRead: 0,
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

/** The chip of a row: "Whole deck runs", "Runs on reads", "9 played by hand", "Cannot be taken" or "Not checked yet". */
export function entrySupport(entry: DeckEntry): { state: DeckState; label: string } {
  if (entry.state === 'blocked') return { state: 'blocked', label: 'Cannot be taken' }
  if (entry.state === 'unchecked') return { state: 'unchecked', label: 'Not checked yet' }
  if (entry.state === 'read') return { state: 'read', label: RUNS_ON_READS }
  if (entry.state === 'verified' && entry.byHand === 0) return { state: 'verified', label: 'Whole deck runs' }
  return { state: 'manual_play', label: `${entry.byHand} ${BY_HAND}` }
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
    if (filter.support === 'verified')
      return (entry.state === 'verified' || entry.state === 'read') && entry.byHand === 0
    return entry.byHand > 0
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

/** How the list is ordered: as the server sends it, by name, or the decks with the fewest cards played by hand first. */
export type SortKey = 'default' | 'name' | 'hand'

/** The orders a list can take, in the order of the choice. */
export const SORT_CHOICES: readonly { value: SortKey; label: string }[] = [
  { value: 'default', label: 'Default order' },
  { value: 'name', label: 'Name' },
  { value: 'hand', label: `Fewest ${BY_HAND}` },
]

/** Whether a stored sort key is one of the orders. */
export const isSortKey = (value: unknown): value is SortKey => SORT_CHOICES.some((c) => c.value === value)

const byName = (a: DeckEntry, b: DeckEntry): number =>
  a.name.localeCompare(b.name, undefined, { sensitivity: 'base', numeric: true }) || a.id.localeCompare(b.id)

/**
 * The entries in the order asked for, without changing the list. Fewest played by hand puts the decks the table cannot
 * take last, since a blocked deck with few cards by hand is of no use, then orders by that count and by name.
 */
export function sortEntries(entries: readonly DeckEntry[], sort: SortKey): DeckEntry[] {
  if (sort === 'default') return [...entries]
  if (sort === 'name') return [...entries].sort(byName)
  const blocked = (entry: DeckEntry): number => (entry.state === 'blocked' ? 1 : 0)
  return [...entries].sort((a, b) => blocked(a) - blocked(b) || a.byHand - b.byHand || byName(a, b))
}

/**
 * The count of cards played by hand in plain words, for a row whose chip does not say it ("Runs on reads", "Cannot be
 * taken", "Not checked yet"); null where the chip already does, or where the game runs the whole deck.
 */
export function handWords(entry: DeckEntry, always: boolean): string | null {
  if (entry.counts === null || entry.state === 'verified' || entry.state === 'manual_play') return null
  return entry.byHand > 0 || always ? cardsByHand(entry.byHand) : null
}
