/** A stored deck whose report names each card's support, for tests of what the builder shows of the check. */
import { legalityIssue, lineOf, reportCard, reportOf } from '@/features/decks/fakeDecks'
import type { CardHit, DeckLineView, DeckView, Reason, ReadState, Support } from '@/protocol/generated'
import { BOLT, DURESS, ELVES, GROWTH, MOUNTAIN, SOL_RING } from './renderBuilder'

/** A clause of a card's text the game leaves to the table. */
export const reason = (text: string, face = 0): Reason => ({ face, span: { start: 0, end: 10 }, text })

/** One card of a checked deck. */
export interface Entry {
  card: CardHit
  count: number
  support: Support
  zone?: 'main' | 'sideboard' | 'commander'
  reasons?: Reason[]
  /** The read state of the draft that decides the card. */
  draft?: ReadState
}

/** Twenty Mountains and a card of each state: Partial, Manual, Stale and Unplayable, and Duress in the sideboard. */
export const ENTRIES: Entry[] = [
  { card: MOUNTAIN, count: 20, support: 'verified' },
  { card: BOLT, count: 4, support: 'partial', reasons: [reason('the engine does not run X yet')] },
  { card: GROWTH, count: 2, support: 'manual', reasons: [reason('no rules for this card yet')] },
  {
    card: SOL_RING,
    count: 1,
    support: 'stale',
    reasons: [reason('the oracle text changed since the rules')],
  },
  { card: ELVES, count: 1, support: 'unplayable', reasons: [reason('the card cannot be hosted')] },
  { card: DURESS, count: 2, support: 'verified', zone: 'sideboard' },
]

/** The fields of a view for `entries` with the report the server would work out; `issues` are the legality sentences. */
export function checked(
  entries: Entry[] = ENTRIES,
  issues: string[] = [],
  over: Partial<DeckView> = {},
): Partial<DeckView> {
  const cards: DeckLineView[] = entries.map((entry) =>
    lineOf(entry.card.name, entry.count, {
      zone: entry.zone ?? 'main',
      card: { ...entry.card, support: entry.support },
    }),
  )
  const report = reportOf(
    entries.map((entry) =>
      reportCard(entry.card.name, {
        oracle_id: entry.card.oracle_id,
        zone: entry.zone ?? 'main',
        count: entry.count,
        support: entry.support,
        reasons: entry.reasons ?? [],
        draft: entry.draft,
      }),
    ),
    issues.map((text) => legalityIssue(text)),
    { format: 'modern' },
  )
  return { cards, report, state: report.summary.state, verified: false, ...over }
}
