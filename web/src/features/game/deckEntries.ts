/**
 * The decks the chooser lists as one kind of row, whatever they come from (stored deck, precon, built-in deck), and
 * the search and filters over them. Pure, so the rows' words and the filters are tested without a page.
 */
import { BY_HAND, byHandCopies, cardsByHand } from '@/lib/byHand'
import { runsBreakdown, WHOLE_DECK_RUNS } from '@/lib/toRead'
import type { ReadTarget } from '@/lib/toRead'
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
  /** Copies the game leaves to the players (partial, manual, stale and unplayable): what "fewest played by hand" orders by. */
  byHand: number
  /** A precon the account starred. */
  favourite: boolean
  /** The deck's own check state; an old `blocked` reads as played by hand. */
  state: DeckState
  /** The deck breaks no rule of its own format; a deck that does still plays, with a warning. */
  legal: boolean
  /** How many rules of its format the deck breaks. */
  issues: number
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

// A card the game cannot play is played by hand too: such a deck still goes to a game.
const handCopies = (counts: Counts): number => byHandCopies(counts) + counts.unplayable

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
    byHand: handCopies(deck.summary.counts),
    favourite: false,
    state: deck.state,
    legal: deck.summary.legal,
    issues: deck.summary.issues,
    toRead: deck.summary.to_read,
    set: null,
    extra: '',
    listed: [],
  }
}

/** A precon as a row; the commanders' pictures and identity come with the list item. */
export function entryOfPrecon(precon: PreconListItem, favourite = false): DeckEntry {
  const commanded = precon.commanders.length > 0
  return {
    id: `precon:${precon.code}`,
    kind: 'precon',
    name: precon.name,
    format: precon.format,
    cards: precon.card_count,
    commanders: commanded ? precon.pictures.map(pictureOf) : [],
    commanderNames: precon.commanders,
    pictures: precon.pictures.map(pictureOf),
    identity: commanded ? identityOf([precon.color_identity]) : null,
    counts: precon.summary.counts,
    byHand: handCopies(precon.summary.counts),
    favourite,
    state: precon.state,
    legal: precon.summary.legal,
    issues: precon.summary.issues,
    toRead: precon.summary.to_read,
    set: precon.set,
    extra: precon.deck_type ?? '',
    listed: [],
  }
}

const COLOUR_LETTER: Record<DeckInfo['color'], string> = {
  White: 'W',
  Blue: 'U',
  Black: 'B',
  Red: 'R',
  Green: 'G',
}

/** The review screen of a row: a stored deck by id, a precon by code; none for a built-in deck, which has no drafts. */
export function readTargetOf(entry: Pick<DeckEntry, 'id' | 'kind'>): ReadTarget | undefined {
  if (entry.kind === 'own') return { deck: entry.id }
  if (entry.kind === 'precon') return { precon: entry.id.slice('precon:'.length) }
  return undefined
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
    identity: [COLOUR_LETTER[deck.color]],
    counts: null,
    byHand: 0,
    favourite: false,
    state: 'verified',
    legal: true,
    issues: 0,
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

/** The chip of a row: "Whole deck runs" (Verified or not), "9 played by hand" or "Not checked yet". */
export function entrySupport(entry: DeckEntry): { state: DeckState; label: string } {
  if (entry.state === 'unchecked') return { state: 'unchecked', label: 'Not checked yet' }
  if (entry.state === 'read') return { state: 'read', label: WHOLE_DECK_RUNS }
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

/** How the list is ordered: as sent, by name, the fewest cards played by hand first, or the starred precons first. */
export type SortKey = 'default' | 'name' | 'hand' | 'favorites'

/** The orders a list can take, in the order of the choice. */
export const SORT_CHOICES: readonly { value: SortKey; label: string }[] = [
  { value: 'default', label: 'Default order' },
  { value: 'name', label: 'Name' },
  { value: 'hand', label: `Fewest ${BY_HAND}` },
  { value: 'favorites', label: 'Favorites first' },
]

/** Whether a stored sort key is one of the orders. */
export const isSortKey = (value: unknown): value is SortKey => SORT_CHOICES.some((c) => c.value === value)

const byName = (a: DeckEntry, b: DeckEntry): number =>
  a.name.localeCompare(b.name, undefined, { sensitivity: 'base', numeric: true }) || a.id.localeCompare(b.id)

/** The entries in the order asked for, without changing the list: fewest played by hand orders by that count and by name. */
export function sortEntries(entries: readonly DeckEntry[], sort: SortKey): DeckEntry[] {
  if (sort === 'default') return [...entries]
  if (sort === 'name') return [...entries].sort(byName)
  if (sort === 'favorites') return [...entries].sort((a, b) => Number(b.favourite) - Number(a.favourite))
  return [...entries].sort((a, b) => a.byHand - b.byHand || byName(a, b))
}

/**
 * The quiet line beside a row's chip: for a deck that runs whole without being all Verified, its other copies by tier
 * ("34 from Forge · 120 unverified · 2 read"); for a row whose chip does not count them ("Not checked yet"), the cards
 * played by hand; null where the chip already says it all.
 */
export function handWords(entry: DeckEntry, always: boolean): string | null {
  if (entry.counts === null || entry.state === 'verified') return null
  if (entry.state === 'manual_play' || entry.state === 'blocked') return null
  if (entry.state === 'read') return runsBreakdown(entry.counts) || null
  return entry.byHand > 0 || always ? cardsByHand(entry.byHand) : null
}
