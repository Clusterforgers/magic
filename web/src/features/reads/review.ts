/** The review screen's items, progress, states and keys, pure (docs/web.md, Reviewing cards). */
import type { FlaggedCard } from '@/features/decks/unvalidated/model'
import type { ReadQueueCard, ReadVerdict } from '@/protocol/generated'
import { BY_HAND, BY_HAND_TITLE, PARTLY_BY_HAND } from '@/lib/byHand'
import { STATE_WORDS } from './model'

/** One card of the review: a draft to read, or a card the game plays partly or wholly by hand to check. */
export type ReviewItem =
  | { kind: 'read'; id: string; name: string; card: ReadQueueCard }
  | { kind: 'check'; id: string; name: string; card: FlaggedCard }

/** What the reader did with an item: a verdict on a draft, or `checked` for a card to check. */
export type Outcome = ReadVerdict | 'checked'

/** The items of a review: every card to read in queue order, then each card to check that is not also to read. */
export function reviewItems(reads: readonly ReadQueueCard[], checks: readonly FlaggedCard[]): ReviewItem[] {
  const seen = new Set<string>()
  const items: ReviewItem[] = []
  for (const card of reads) {
    if (seen.has(card.oracle_id)) continue
    seen.add(card.oracle_id)
    items.push({ kind: 'read', id: card.oracle_id, name: card.name, card })
  }
  for (const card of checks) {
    if (seen.has(card.oracleId)) continue
    seen.add(card.oracleId)
    items.push({ kind: 'check', id: card.oracleId, name: card.name, card })
  }
  return items
}

/** The item after `current` that has no outcome yet, wrapping around; null when none is left. */
export function nextOpen(
  items: readonly ReviewItem[],
  done: ReadonlyMap<string, Outcome>,
  current: string | null,
): string | null {
  const start = current === null ? -1 : items.findIndex((item) => item.id === current)
  for (let step = 1; step <= items.length; step += 1) {
    const item = items[(start + step + items.length) % items.length]
    if (item !== undefined && !done.has(item.id)) return item.id
  }
  return null
}

/** How far the review is: items with an outcome, all items, and "5 of 12 reviewed". */
export function progressOf(
  items: readonly ReviewItem[],
  done: ReadonlyMap<string, Outcome>,
): { done: number; total: number; text: string } {
  const finished = items.filter((item) => done.has(item.id)).length
  return { done: finished, total: items.length, text: `${finished} of ${items.length} reviewed` }
}

/** "3 to read, 2 to check.": the count line of the cards to read with what is left to check, when any is. */
export function reviewLine(readLine: string, checks: number): string {
  return checks === 0 ? readLine : `${readLine.replace(/\.$/, '')}, ${checks} to check.`
}

/** The tone of an item's state in the list: the page picks the icon and colour by it, the words say it. */
export type RowTone = 'to_read' | 'right' | 'wrong' | 'by_hand' | 'checked'

/** A short state of a card to check, by its support. */
export const CHECK_WORDS: Readonly<Record<FlaggedCard['support'], string>> = {
  partial: PARTLY_BY_HAND.charAt(0).toUpperCase() + PARTLY_BY_HAND.slice(1),
  manual: BY_HAND_TITLE,
  stale: `Text changed, ${BY_HAND}`,
  unplayable: 'The game cannot play it',
}

/** The state of an item in the list, in words and a tone. */
export function rowState(item: ReviewItem, outcome: Outcome | undefined): { tone: RowTone; words: string } {
  if (outcome === 'correct') return { tone: 'right', words: 'You said it reads right' }
  if (outcome === 'wrong') return { tone: 'wrong', words: 'You said it reads wrong' }
  if (outcome === 'checked') return { tone: 'checked', words: 'Checked' }
  return item.kind === 'read'
    ? { tone: 'to_read', words: STATE_WORDS[item.card.state] }
    : { tone: 'by_hand', words: CHECK_WORDS[item.card.support] }
}

/** A key of the review screen besides the verdicts: the next or previous card, play by hand, or the list of keys. */
export type ReviewKey = 'next' | 'previous' | 'by_hand' | 'help'

/** What a key press asks of the review screen, outside a text field and with no modifier but Shift; null for none. */
export function reviewKey(
  event: Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'altKey' | 'target'>,
): ReviewKey | null {
  if (event.ctrlKey || event.metaKey || event.altKey) return null
  const target = event.target
  if (
    target instanceof HTMLElement &&
    (target.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName))
  )
    return null
  switch (event.key) {
    case 'j':
    case 'J':
    case 'ArrowRight':
      return 'next'
    case 'k':
    case 'K':
    case 'ArrowLeft':
      return 'previous'
    case 'h':
    case 'H':
      return 'by_hand'
    case '?':
      return 'help'
    default:
      return null
  }
}

/** The keys of the review screen and what each does, in the order the list of keys shows them. */
export const REVIEW_KEYS: ReadonlyArray<{ keys: readonly string[]; does: string }> = [
  { keys: ['C'], does: 'Reads right, or checked' },
  { keys: ['W'], does: 'Reads wrong (asks for a note)' },
  { keys: ['J', '→'], does: 'Next card' },
  { keys: ['K', '←'], does: 'Previous card' },
  { keys: ['H'], does: 'Play by hand in the card’s only deck' },
  { keys: ['?'], does: 'Show or hide these keys' },
]

const CHECKED_KEY = 'mtg.review-checked:'

/** The cards the reader marked checked in this queue, as the browser kept them; none when storage is off. */
export function savedChecked(queue: string): string[] {
  try {
    const raw = window.localStorage.getItem(CHECKED_KEY + queue)
    const value: unknown = raw === null ? [] : JSON.parse(raw)
    return Array.isArray(value) ? value.filter((id): id is string => typeof id === 'string') : []
  } catch {
    return []
  }
}

/** Keeps the cards checked in this queue, so a reload shows the same progress. */
export function saveChecked(queue: string, ids: readonly string[]): void {
  try {
    window.localStorage.setItem(CHECKED_KEY + queue, JSON.stringify(ids))
  } catch {
    // Without storage a reload shows the checked cards open again; nothing the server holds is lost.
  }
}
