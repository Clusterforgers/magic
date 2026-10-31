/**
 * The draft as the deck views read it: the lines with the check's verdict as the cards of a deck preview (so the shared
 * grouping, curve and counts apply), and the cards the game leaves to the table with their reasons. Pure, so tables test it.
 */
import { KIND_LABEL, KIND_ORDER, kindOf } from '@/features/decks/preview/model'
import type { CardKind, DeckPreviewData, PreviewCard } from '@/features/decks/preview/model'
import { byHandCopies, cardsByHand, runsWhole } from '@/lib/byHand'
import type { CardHit, Counts, ReadState, Support } from '@/protocol/generated'
import type { Draft, Line } from './deckEdit'
import { checkOf, reasonLines, SUPPORT_ORDER } from './report'
import type { LineCheck, SortKey } from './report'

/** One line as a card of the preview: its copies, type, cost, identity and the check's support and reasons. */
export function previewCardOf(line: Line, check: LineCheck): PreviewCard {
  const { card, count } = line
  return {
    oracleId: card.oracle_id,
    name: card.name,
    count,
    typeLine: card.type_line,
    manaValue: card.mana_value,
    manaCost: card.mana_cost,
    identity: card.color_identity,
    support: check.support,
    draft: check.draft,
    reasons: check.support === 'verified' ? [] : reasonLines(check.reasons).map((reason) => reason.text),
  }
}

/** The draft as the preview takes it: the command zone, the main deck with a companion, and the sideboard. */
export function previewFromDraft(draft: Draft, checks: ReadonlyMap<string, LineCheck>): DeckPreviewData {
  const data: DeckPreviewData = {
    name: draft.name,
    source: 'Your deck',
    format: draft.format,
    commanders: [],
    cards: [],
    sideboard: [],
  }
  for (const line of draft.lines) {
    const board =
      line.zone === 'commander' ? data.commanders : line.zone === 'sideboard' ? data.sideboard : data.cards
    board.push(previewCardOf(line, checkOf(checks, line)))
  }
  return data
}

/** The lines of one type group of a zone. */
export interface LineGroup {
  kind: CardKind
  label: string
  /** Copies in the group. */
  count: number
  lines: Line[]
}

/**
 * The lines in groups by type (`KIND_ORDER`, an empty group left out). With the name sort a group runs by mana value and
 * then name, the way a deck is read; with the issues sort the order the lines come in is kept, so the cards that need
 * attention stay first.
 */
export function groupLines(lines: readonly Line[], sort: SortKey): LineGroup[] {
  return KIND_ORDER.flatMap((kind) => {
    const inKind = lines.filter((line) => kindOf(line.card.type_line) === kind)
    if (inKind.length === 0) return []
    if (sort === 'name') {
      inKind.sort(
        (a, b) => a.card.mana_value - b.card.mana_value || a.card.name.localeCompare(b.card.name, 'en'),
      )
    }
    return [
      {
        kind,
        label: KIND_LABEL[kind],
        count: inKind.reduce((sum, line) => sum + line.count, 0),
        lines: inKind,
      },
    ]
  })
}

/** A card the game does not run whole, once, with its copies in every zone. */
export interface HandCard {
  card: CardHit
  /** Copies in the whole deck. */
  count: number
  support: Support
  /** The read state of the draft that decides the card, once it has one. */
  draft?: ReadState
  /** Why the game leaves the card to the table, as sentences; empty when the check has not seen the card yet. */
  reasons: readonly string[]
}

/** The cards that are not Verified, worst state first and then by name; a card in two zones is one entry. */
export function handCards(draft: Draft, checks: ReadonlyMap<string, LineCheck>): HandCard[] {
  const found = new Map<string, HandCard>()
  for (const line of draft.lines) {
    const check = checkOf(checks, line)
    if (runsWhole(check.support)) continue
    const kept = found.get(line.card.oracle_id)
    if (kept === undefined) {
      found.set(line.card.oracle_id, {
        card: line.card,
        count: line.count,
        support: check.support,
        draft: check.draft,
        reasons: reasonLines(check.reasons).map((reason) => reason.text),
      })
    } else {
      kept.count += line.count
    }
  }
  const rank = (support: Support): number => SUPPORT_ORDER.indexOf(support)
  return [...found.values()].sort(
    (a, b) => rank(a.support) - rank(b.support) || a.card.name.localeCompare(b.card.name, 'en'),
  )
}

/** "7 cards played by hand", "1 card played by hand, 1 cannot be played" or "The whole deck runs." */
export function handSentence(counts: Counts): string {
  const hand = byHandCopies(counts)
  const { unplayable } = counts
  if (hand === 0 && unplayable === 0)
    return counts.total === 0 ? 'No cards to count.' : 'The whole deck runs.'
  return [hand === 0 ? null : cardsByHand(hand), unplayable === 0 ? null : `${unplayable} cannot be played`]
    .filter((part) => part !== null)
    .join(', ')
}
