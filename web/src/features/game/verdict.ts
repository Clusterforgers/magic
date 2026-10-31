/**
 * What a deck means at this table, from `POST /api/decks/check` with the game: the whole deck runs, some cards are
 * played by hand, or the table cannot take it, with the reasons. Pure, so the wording is tested without a page.
 */
import { issueWords } from '@/features/game/deckWords'
import { cardsByHand, runsWhole } from '@/lib/byHand'
import type { DeckReport, DeckState } from '@/protocol/generated'

/** The verdict of one deck at one table. */
export type Verdict =
  { kind: 'runs' } | { kind: 'hand'; cards: number; names: string[] } | { kind: 'blocked'; reasons: string[] }

const plural = (n: number, one: string, many = `${one}s`): string => `${n} ${n === 1 ? one : many}`

/** The verdict a report gives: blocked on any issue at the table or any card the game cannot play. */
export function verdictOf(report: DeckReport): Verdict {
  const { counts, state } = report.summary
  const reasons = report.legality.map(issueWords)
  if (counts.unplayable > 0) reasons.push(`${plural(counts.unplayable, 'card')} the game cannot play.`)
  if (state === 'blocked' && reasons.length === 0) reasons.push('The deck cannot be played as it stands.')
  if (reasons.length > 0) return { kind: 'blocked', reasons }
  const cards = counts.manual + counts.partial + counts.stale
  if (cards === 0) return { kind: 'runs' }
  const names = [...new Set(report.cards.filter((card) => !runsWhole(card.support)).map((card) => card.name))]
  return { kind: 'hand', cards, names }
}

/** The state the verdict's badge shows: the words of the rest of the web (icon and text) with the verdict's label. */
export function verdictState(verdict: Verdict): DeckState {
  return verdict.kind === 'runs' ? 'verified' : verdict.kind === 'hand' ? 'manual_play' : 'blocked'
}

/** "Whole deck runs", "3 cards played by hand" or "Cannot be taken". */
export function verdictLabel(verdict: Verdict): string {
  switch (verdict.kind) {
    case 'runs':
      return 'Whole deck runs'
    case 'hand':
      return cardsByHand(verdict.cards)
    case 'blocked':
      return 'Cannot be taken'
  }
}
