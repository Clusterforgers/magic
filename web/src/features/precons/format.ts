/** The precon browser as plain data: a deck's check in words, the filters, the dates and the copyable list. Pure, so tables test it. */
import { cardsText } from '@/features/decks/format'
import type { Counts, DeckZone, PreconListItem, PreconView, Reason, Support } from '@/protocol/generated'
import { reasonLines } from '@/features/builder/report'

/** How a game treats a precon: it runs the whole deck, it plays some cards by hand, or it cannot take the deck. */
export type PreconStatus = 'whole' | 'hand' | 'blocked'

/** The statuses from the best to the worst, which the state filter lists in this order. */
export const STATUSES: readonly PreconStatus[] = ['whole', 'hand', 'blocked']

/** The status of a precon: `blocked` when the server's `playable` is false or the check blocks it. */
export function statusOf(precon: Pick<PreconListItem, 'state' | 'playable'>): PreconStatus {
  if (!precon.playable || precon.state === 'blocked') return 'blocked'
  return precon.state === 'verified' ? 'whole' : 'hand'
}

/** Copies the game plays by hand: the cards that are partial, manual or stale. */
export const byHandCopies = (counts: Counts): number => counts.partial + counts.manual + counts.stale

/** The words of a status for the state filter. */
export const STATUS_LABEL: Readonly<Record<PreconStatus, string>> = {
  whole: 'Whole deck runs',
  hand: 'Some cards by hand',
  blocked: 'Cannot be taken',
}

/** The badge's words: "Whole deck runs", "3 cards played by hand" or "Cannot be taken". */
export function statusWords(precon: PreconListItem): string {
  const status = statusOf(precon)
  if (status !== 'hand') return STATUS_LABEL[status]
  const copies = byHandCopies(precon.summary.counts)
  return `${copies} ${copies === 1 ? 'card' : 'cards'} played by hand`
}

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
  /** One status, or null for every one. */
  status: PreconStatus | null
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
    if (filters.status !== null && statusOf(item) !== filters.status) return false
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
  const parts = [
    counts.partial > 0 ? `${counts.partial} partial` : null,
    counts.manual > 0 ? `${counts.manual} manual` : null,
    counts.stale > 0 ? `${counts.stale} stale` : null,
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

// What a card with no clause of its own says, by state: the card is still played by hand.
const NO_CLAUSE: Readonly<Record<Support, string>> = {
  verified: '',
  partial: 'partly played by hand',
  manual: 'played by hand',
  stale: 'its text changed since the rules were written, so it is played by hand',
  unplayable: 'the game cannot play it',
}

/** The unread clauses of a card as one phrase: the reasons joined with "; ", or the state's words when it has none. */
export function clausesText(support: Support, reasons: readonly Reason[]): string {
  const lines = reasonLines(reasons).map((line) => line.text.replace(/\.$/, ''))
  return lines.length > 0 ? lines.join('; ') : NO_CLAUSE[support]
}

/**
 * The cards that are not Verified as plain text, one line a card (`Name: clause; clause`), in the order of the boards.
 * Empty when every card is verified.
 */
export function copyList(view: PreconView): string {
  return view.boards
    .flatMap((board) => board.cards)
    .filter((card) => card.support !== 'verified')
    .map((card) => `${card.name}: ${clausesText(card.support, card.reasons)}`)
    .join('\n')
}

/** How many lines `copyList` writes. */
export const notVerified = (view: PreconView): number =>
  view.boards.reduce(
    (sum, board) => sum + board.cards.filter((card) => card.support !== 'verified').length,
    0,
  )
