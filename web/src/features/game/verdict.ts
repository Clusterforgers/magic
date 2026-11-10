/**
 * What a deck means at this table, from `POST /api/decks/check` with the game: the whole deck runs or some cards are
 * played by hand, and apart from that whether it breaks the table's rules, as a warning that never keeps it out. Pure,
 * so the wording is tested without a page.
 */
import { issueWords } from '@/features/game/deckWords'
import { cardsByHand, runsWhole } from '@/lib/byHand'
import { WHOLE_DECK_RUNS } from '@/lib/toRead'
import type { DeckReport, DeckState, Issue } from '@/protocol/generated'

/** How the game runs the deck at this table. */
export type Play =
  | { kind: 'runs' }
  | { kind: 'reads'; cards: number }
  | { kind: 'hand'; cards: number; names: string[]; toRead: number }

/** The rules of the table the deck breaks: the issues to group and each as a sentence. */
export interface TableWarning {
  issues: Pick<Issue, 'code'>[]
  reasons: string[]
}

/** The verdict of one deck at one table: how it plays, and its legality warning (null when it is legal here). */
export type Verdict = Play & { warning: TableWarning | null }

/** The verdict a report gives. A card the game cannot play is played by hand; legality only warns. */
export function verdictOf(report: DeckReport): Verdict {
  const { counts } = report.summary
  const warning =
    report.legality.length === 0
      ? null
      : { issues: report.legality.map(({ code }) => ({ code })), reasons: report.legality.map(issueWords) }
  const cards = counts.manual + counts.partial + counts.stale + counts.unplayable
  if (cards === 0) {
    const read = counts.read + counts.forge + counts.unverified
    return read === 0 ? { kind: 'runs', warning } : { kind: 'reads', cards: read, warning }
  }
  const names = [...new Set(report.cards.filter((card) => !runsWhole(card.support)).map((card) => card.name))]
  return { kind: 'hand', cards, names, toRead: report.summary.to_read, warning }
}

/** The state the verdict's badge shows: the words of the rest of the web (icon and text) with the verdict's label. */
export function verdictState(verdict: Play): DeckState {
  switch (verdict.kind) {
    case 'runs':
      return 'verified'
    case 'reads':
      return 'read'
    case 'hand':
      return 'manual_play'
  }
}

/** "Whole deck runs" (Verified or not) or "3 cards played by hand". */
export function verdictLabel(verdict: Play): string {
  switch (verdict.kind) {
    case 'runs':
      return 'Whole deck runs'
    case 'reads':
      return WHOLE_DECK_RUNS
    case 'hand':
      return cardsByHand(verdict.cards)
  }
}
