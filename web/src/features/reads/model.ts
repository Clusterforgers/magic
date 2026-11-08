/** The words and rules of the review screen, pure (docs/web.md, Reading cards). */
import type { ReadCard, ReadQueueCard, ReadRow, ReadState } from '@/protocol/generated'
import { BY_HAND_TITLE } from '@/lib/byHand'
import { RUNS_ON_READS } from '@/lib/toRead'
import type { ReadTarget } from '@/lib/toRead'

/** How long a card is shown before a verdict can be given, so nobody judges a card without reading it. */
export const READ_DELAY_MS = 2000

/** The longest note of a `wrong` verdict, in characters: the server's and the fix marks' limit. */
export const MAX_READ_NOTE_CHARS = 280

/** How a row of the sheet stands, in one word the page styles by. */
export type RowTone = 'written' | 'unwritten' | 'differs' | 'by_hand' | 'no_clause'

/** The tone of a row. */
export function toneOf(row: ReadRow): RowTone {
  const state = row.state
  if (typeof state === 'string') return state
  if ('differs' in state) return 'differs'
  if ('unwritten' in state) return 'unwritten'
  return 'by_hand'
}

/** What the row's tone says, beside the engine's reading. */
export const TONE_WORDS: Readonly<Record<RowTone, string>> = {
  written: 'Reads back as printed',
  unwritten: 'No wording to compare; read the steps',
  differs: 'Worded differently: compare closely',
  by_hand: BY_HAND_TITLE,
  no_clause: 'Not on the printed card',
}

/** The engine's own English for a row: its writer's wording, else the wording it differs with, else none. */
export function engineWords(row: ReadRow): string | null {
  if (row.wording !== null) return row.wording
  const state = row.state
  if (typeof state !== 'string' && 'differs' in state) return state.differs.written
  return null
}

/** Why a by-hand row is played by hand, when the rules say so. */
export function byHandReason(row: ReadRow): string | null {
  const state = row.state
  return typeof state !== 'string' && 'by_hand' in state ? state.by_hand.reason : null
}

/** How many rows of a sheet read back as other text. */
export function differsCount(card: ReadCard): number {
  return card.sheet.rows.filter((row) => toneOf(row) === 'differs').length
}

/** The words of a draft's state. */
export const STATE_WORDS: Readonly<Record<ReadState, string>> = {
  pending: 'Waits for a read',
  approved: RUNS_ON_READS,
  wrong: 'Found wrong',
}

/** "Found correct by 2 players", "Found wrong once": the counts on the draft's hash, or null when none. */
export function countsLine(card: Pick<ReadCard, 'correct' | 'wrong' | 'forge'>): string | null {
  const parts: string[] = []
  const players = (count: number): string => (count === 1 ? '1 player' : `${count} players`)
  if (card.correct > 0) parts.push(`found correct by ${players(card.correct)}`)
  if (card.wrong > 0) parts.push(`found wrong ${card.wrong === 1 ? 'once' : `${card.wrong} times`}`)
  if (parts.length === 0) return card.forge ? 'Translated from Forge; nobody has read it yet' : null
  const line = parts.join(', ')
  return line.charAt(0).toUpperCase() + line.slice(1)
}

/** What is wrong with a note of a `wrong` verdict, or null when it can be sent. */
export function noteProblem(note: string): string | null {
  const trimmed = note.trim()
  if (trimmed === '') return 'Say what the rules do wrong.'
  if (Array.from(trimmed).length > MAX_READ_NOTE_CHARS)
    return `Keep the note to ${MAX_READ_NOTE_CHARS} characters.`
  // eslint-disable-next-line no-control-regex
  if (/[\u0000-\u0009\u000b-\u001f\u007f]/.test(trimmed)) return 'Remove the tab or control character.'
  return null
}

/** The address parameters of `/read`, read leniently: a deck id or a precon code, the deck first. */
export function parseReadSearch(search: Record<string, unknown>): { deck?: string; precon?: string } {
  const text = (value: unknown): string | undefined =>
    typeof value === 'string' && value.trim() !== '' ? value.trim() : undefined
  const deck = text(search.deck)
  const precon = deck === undefined ? text(search.precon) : undefined
  return { deck, precon }
}

/** The deck the review screen reads from its address, or null when it names none. */
export function targetOf(search: { deck?: string; precon?: string }): ReadTarget | null {
  if (search.deck !== undefined) return { deck: search.deck }
  if (search.precon !== undefined) return { precon: search.precon }
  return null
}

/** The card after `current` in the list that the reader has not judged yet, wrapping around; null when none is left. */
export function nextToRead(
  cards: readonly ReadQueueCard[],
  done: ReadonlySet<string>,
  current: string | null,
): string | null {
  const start = current === null ? -1 : cards.findIndex((card) => card.oracle_id === current)
  for (let step = 1; step <= cards.length; step += 1) {
    const card = cards[(start + step + cards.length) % cards.length]
    if (card !== undefined && !done.has(card.oracle_id)) return card.oracle_id
  }
  return null
}

/** Whether a key press stands for a verdict: C or W outside a text field, with no modifier. */
export function verdictKey(
  event: Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'altKey' | 'target'>,
): 'correct' | 'wrong' | null {
  if (event.ctrlKey || event.metaKey || event.altKey) return null
  const target = event.target
  if (
    target instanceof HTMLElement &&
    (target.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName))
  )
    return null
  const key = event.key.toLowerCase()
  if (key === 'c') return 'correct'
  if (key === 'w') return 'wrong'
  return null
}
