/** The precon browser as plain data: a deck's check in words, the filters, the dates and the copyable list. Pure, so tables test it. */
import { cardsText } from '@/features/decks/format'
import { notLegalText } from '@/features/decks/legality'
import { BY_HAND, byHandCopies, cardsByHand, runsWhole } from '@/lib/byHand'
import { runsBreakdown, WHOLE_DECK_RUNS } from '@/lib/toRead'
import { copyText } from '@/features/decks/unvalidated/model'
import type { FaceTexts, FlaggedCard } from '@/features/decks/unvalidated/model'
import type { DeckZone, PreconCardView, PreconListItem, PreconView } from '@/protocol/generated'

/** How a game runs a precon: the whole deck (`read`: some of it not Verified), or with cards played by hand. */
export type PreconStatus = 'whole' | 'read' | 'hand'

/** What the deck-check filter narrows to: a status, or the precons that are not legal in their format. */
export type StatusFilter = PreconStatus | 'illegal'

/** The filter's choices: the statuses from the best to the worst, then the legality warning. */
export const STATUSES: readonly StatusFilter[] = ['whole', 'read', 'hand', 'illegal']

/** The status of a precon; an old `blocked` check is played by hand. Legality is apart (`summary.legal`). */
export function statusOf(precon: Pick<PreconListItem, 'state'>): PreconStatus {
  if (precon.state === 'verified') return 'whole'
  return precon.state === 'read' ? 'read' : 'hand'
}

/** The words of a filter choice, which tells a deck of Verified cards from one with others that run. */
export const STATUS_LABEL: Readonly<Record<StatusFilter, string>> = {
  whole: `${WHOLE_DECK_RUNS}, all verified`,
  read: `${WHOLE_DECK_RUNS}, not all verified`,
  hand: `Some cards ${BY_HAND}`,
  illegal: 'Not legal in its format',
}

/** Copies of a precon played by hand, the ones the game cannot play among them. */
const handCopies = (precon: PreconListItem): number =>
  byHandCopies(precon.summary.counts) + precon.summary.counts.unplayable

/** The badge's words: "Whole deck runs" or "3 cards played by hand". */
export function statusWords(precon: PreconListItem): string {
  const status = statusOf(precon)
  if (status === 'whole' || status === 'read') return WHOLE_DECK_RUNS
  return cardsByHand(handCopies(precon))
}

/** The warning of a precon that breaks its format's rules ("Not legal in Commander: 2 issues"); null for a legal one. */
export const legalityWords = (precon: PreconListItem): string | null =>
  precon.summary.legal ? null : notLegalText(precon.format, precon.summary.issues)

/** The quiet line beside the badge of a deck that runs whole: "34 from Forge · 120 unverified · 2 read"; empty otherwise. */
export const breakdownWords = (precon: PreconListItem): string =>
  statusOf(precon) === 'read' ? runsBreakdown(precon.summary.counts) : ''

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'] as const

/** "Oct 2018" for `2018-10-05`; "Release date unknown" for none or a date this page cannot read. */
export function releaseText(date: string | null): string {
  const match = date === null ? null : /^(\d{4})-(\d{2})/.exec(date)
  const month = match === null ? undefined : MONTHS[Number(match[2]) - 1]
  return match === null || month === undefined ? 'Release date unknown' : `${month} ${match[1]}`
}

/** The names of the commanders joined with " and " / commas; "No commander" for none. */
export function commandersText(names: readonly string[]): string {
  if (names.length === 0) return 'No commander'
  if (names.length === 1) return names[0] ?? ''
  return `${names.slice(0, -1).join(', ')} and ${names.at(-1) ?? ''}`
}

/** What the filters of the list hold. */
export interface Filters {
  /** Free text: the name, the set or a commander contains it. */
  text: string
  /** One set code, or null for every set. */
  set: string | null
  /** One status or the legality warning, or null for every precon. */
  status: StatusFilter | null
}

/** Nothing filtered. */
export const NO_FILTERS: Filters = { text: '', set: null, status: null }

/** True when a filter is set. */
export const filtersActive = (filters: Filters): boolean =>
  filters.text.trim() !== '' || filters.set !== null || filters.status !== null

const fold = (text: string): string => text.normalize('NFD').replace(/\p{M}/gu, '').toLocaleLowerCase().trim()

/** The precons that match, in the order given: every word of the text appears in the name, set or a commander. */
export function filterPrecons(items: readonly PreconListItem[], filters: Filters): PreconListItem[] {
  const words = fold(filters.text)
    .split(/\s+/)
    .filter((word) => word !== '')
  return items.filter((item) => {
    if (filters.set !== null && item.set !== filters.set) return false
    if (filters.status === 'illegal' && item.summary.legal) return false
    if (filters.status !== null && filters.status !== 'illegal' && statusOf(item) !== filters.status) return false
    const haystack = fold([item.name, item.set, ...item.commanders].join(' '))
    return words.every((word) => haystack.includes(word))
  })
}

/** A set of the list and how many precons it holds. */
export interface SetOption {
  set: string
  count: number
}

/** The sets of the list, newest first (the order the list arrives in), each with its number of precons. */
export function setOptions(items: readonly PreconListItem[]): SetOption[] {
  const counts = new Map<string, number>()
  for (const item of items) counts.set(item.set, (counts.get(item.set) ?? 0) + 1)
  return [...counts].map(([set, count]) => ({ set, count }))
}

/** "198 precons" or "12 of 198 precons". */
export function countLine(shown: number, total: number): string {
  const noun = total === 1 ? 'precon' : 'precons'
  return shown === total ? `${total} ${noun}` : `${shown} of ${total} ${noun}`
}

/** The line under the badge of the details: the cards in the deck and what the check found in them. */
export function checkLine(precon: PreconListItem): string {
  const { counts } = precon.summary
  const hand = byHandCopies(counts)
  const parts = [
    counts.forge > 0 ? `${counts.forge} from Forge` : null,
    counts.unverified > 0 ? `${counts.unverified} unverified` : null,
    counts.read > 0 ? `${counts.read} read` : null,
    hand > 0 ? `${hand} ${BY_HAND}` : null,
    counts.unplayable > 0 ? `${counts.unplayable} unplayable` : null,
  ].filter((part) => part !== null)
  const base = `${counts.verified} of ${cardsText(counts.total)} verified`
  return parts.length === 0 ? base : `${base}; ${parts.join(', ')}`
}

/** The boards of a precon in the order they are read, with the heading of each. */
export const BOARD_LABEL: Readonly<Record<DeckZone, string>> = {
  commander: 'Commander',
  main: 'Main deck',
  sideboard: 'Sideboard',
  companion: 'Companion',
}

/** A line of a precon as an entry of the list of unvalidated cards; null for a Verified card. */
export function flaggedLine(card: PreconCardView, precon: PreconListItem): FlaggedCard | null {
  if (runsWhole(card.support)) return null
  return {
    oracleId: card.card?.oracle_id ?? `name:${card.name}`,
    name: card.name,
    support: card.support,
    reasons: card.reasons,
    decks: [{ id: precon.code, name: precon.name }],
  }
}

/** The cards of a precon that are not Verified, in the order of the boards. */
export const flaggedCards = (view: PreconView): FlaggedCard[] =>
  view.boards.flatMap((board) => board.cards).flatMap((card) => flaggedLine(card, view.precon) ?? [])

/**
 * The cards that are not Verified as plain text in the format of the list of unvalidated cards (a block each, the
 * quoted clause beside its reason once the texts of the cards are known). Empty when every card is verified.
 */
export const copyList = (view: PreconView, texts: FaceTexts = new Map()): string =>
  copyText(flaggedCards(view), texts)

/** How many cards `copyList` writes. */
export const notVerified = (view: PreconView): number => flaggedCards(view).length
