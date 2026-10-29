/**
 * Choosing a commander: the deck check of the server judges whether a card may lead the deck, alone or beside the
 * commanders already chosen (a legendary creature or a card that says it can, a Partner pair, Friends forever, a
 * creature that chooses a Background with its Background, a Doctor's companion with a Time Lord Doctor), so the rules
 * live in one place (`docs/deck-check.md`). The builder sends the command zone with the candidate and refuses it with
 * the sentence of the issue the server names.
 */
import { describeDeckError } from '@/features/decks/errors'
import type { DecksApi } from '@/net/decks'
import type { CardHit, Issue, IssueCode } from '@/protocol/generated'
import { checkRequestOf } from './deckEdit'
import type { Draft, Line } from './deckEdit'
import { sentence } from './report'

/** The issues that make a choice of commander wrong, as opposed to a deck that is merely unfinished. */
export const COMMANDER_CODES: readonly IssueCode[] = [
  'commander_count',
  'commander_eligibility',
  'commander_pair',
]

/** The verdict on a candidate: it may join the command zone, or why it may not. */
export type CommanderVerdict = { ok: true } | { ok: false; reason: string }

/** The sentences of the issues that are about choosing commanders, in the report's order. */
export const commanderReasons = (issues: readonly Issue[]): string[] =>
  issues.filter((issue) => COMMANDER_CODES.includes(issue.code)).map((issue) => sentence(issue.message))

/** The command zone of `draft` with one copy of `card` added, as lines of a check. */
export function withCommander(draft: Draft, card: CardHit): Line[] {
  const kept = draft.lines.filter(
    (line) => line.zone === 'commander' && line.card.oracle_id !== card.oracle_id,
  )
  return [...kept, { zone: 'commander', card, count: 1, printingId: null }]
}

/**
 * Asks the server whether `card` may be a commander of `draft`. A card that already is one is refused here, since the
 * server would call the second copy a pair. A failed check refuses too, with the reason it failed, so a card is never
 * added unchecked.
 */
export async function judgeCommander(
  api: DecksApi,
  draft: Draft,
  card: CardHit,
  signal?: AbortSignal,
): Promise<CommanderVerdict> {
  if (draft.lines.some((line) => line.zone === 'commander' && line.card.oracle_id === card.oracle_id)) {
    return { ok: false, reason: `${card.name} is already a commander of this deck.` }
  }
  try {
    const report = await api.check(
      checkRequestOf({ format: 'commander', lines: withCommander(draft, card) }),
      signal,
    )
    const reasons = commanderReasons(report.legality)
    return reasons.length === 0 ? { ok: true } : { ok: false, reason: reasons.join(' ') }
  } catch (error) {
    return { ok: false, reason: `Could not check ${card.name} as a commander. ${describeDeckError(error)}` }
  }
}
