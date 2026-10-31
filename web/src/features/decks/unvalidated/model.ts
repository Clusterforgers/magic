/**
 * The cards of decks that the game does not run whole, as plain data: which cards, in which decks, with which unread
 * clauses quoted from the oracle text, and the text that lists them. Pure, so tables test it.
 */
import { reasonLines } from '@/features/builder/report'
import { BY_HAND, PARTLY_BY_HAND, runsWhole } from '@/lib/byHand'
import type { Reason, ReadState, ReportCard, Support } from '@/protocol/generated'
import { SUPPORT_LABEL } from '../format'

/** The support states of a card the game does not run whole. */
export type Unvalidated = Exclude<Support, 'verified' | 'read'>

/** The states in the order the filters list them. */
export const UNVALIDATED: readonly Unvalidated[] = ['partial', 'manual', 'stale', 'unplayable']

/** A deck by id and name. */
export interface DeckRef {
  id: string
  name: string
}

/** A deck with the cards its check reported. */
export interface CheckedDeck extends DeckRef {
  cards: readonly ReportCard[]
}

/** A card the game does not run whole, with the decks that hold it. */
export interface FlaggedCard {
  /** The oracle id; a card the database lacks has `name:` and its name. */
  oracleId: string
  name: string
  support: Unvalidated
  /** The clauses left to the table; none for a card with no rules file, whose whole text is left. */
  reasons: readonly Reason[]
  /** The read state of the draft that decides the card, once it has one. */
  draft?: ReadState
  decks: readonly DeckRef[]
}

/** One deck with its flagged cards. */
export interface DeckGroup {
  deck: DeckRef
  cards: FlaggedCard[]
}

/** What a card says when it has no clause of its own: the card is still played by hand. */
export const NO_CLAUSE: Readonly<Record<Support, string>> = {
  verified: '',
  read: '',
  partial: PARTLY_BY_HAND,
  manual: BY_HAND,
  stale: `its text changed since the rules were written, so it is ${BY_HAND}`,
  unplayable: 'the game cannot play it',
}

const isUnvalidated = (support: Support): support is Unvalidated => !runsWhole(support)

const byName = (a: FlaggedCard, b: FlaggedCard): number =>
  a.name.localeCompare(b.name, 'en', { sensitivity: 'base' }) || a.oracleId.localeCompare(b.oracleId)

/** The cards of one deck that are not Verified, one entry per card even when it sits in two zones, by name. */
function flaggedIn(deck: CheckedDeck): FlaggedCard[] {
  const found = new Map<string, FlaggedCard>()
  for (const card of deck.cards) {
    if (!isUnvalidated(card.support) || found.has(card.oracle_id)) continue
    found.set(card.oracle_id, {
      oracleId: card.oracle_id,
      name: card.name,
      support: card.support,
      reasons: card.reasons,
      draft: card.draft,
      decks: [{ id: deck.id, name: deck.name }],
    })
  }
  return [...found.values()].sort(byName)
}

/** Each deck that holds a card the game does not run whole, with those cards by name; decks that run whole are left out. */
export function groupByDeck(decks: readonly CheckedDeck[]): DeckGroup[] {
  return decks.flatMap((deck) => {
    const cards = flaggedIn(deck)
    return cards.length === 0 ? [] : [{ deck: { id: deck.id, name: deck.name }, cards }]
  })
}

/**
 * Every such card once, with all the decks that hold it (in the order of `decks`), the cards held by most decks first,
 * then by name.
 */
export function mergeDecks(decks: readonly CheckedDeck[]): FlaggedCard[] {
  const merged = new Map<string, FlaggedCard>()
  for (const group of groupByDeck(decks)) {
    for (const card of group.cards) {
      const before = merged.get(card.oracleId)
      merged.set(
        card.oracleId,
        before === undefined ? card : { ...before, decks: [...before.decks, group.deck] },
      )
    }
  }
  return [...merged.values()].sort((a, b) => b.decks.length - a.decks.length || byName(a, b))
}

/** A filter of the list: every card, those whose draft waits for a read, or those in one state. */
export type StateFilter = 'all' | 'to_read' | Unvalidated

/** Whether a card's draft waits for a player to read it. */
export const waitsForRead = (card: { draft?: ReadState }): boolean => card.draft === 'pending'

/** The cards in `filter`'s state. */
export function filterCards<T extends { support: Unvalidated; draft?: ReadState }>(
  cards: readonly T[],
  filter: StateFilter,
): T[] {
  if (filter === 'all') return [...cards]
  if (filter === 'to_read') return cards.filter(waitsForRead)
  return cards.filter((card) => card.support === filter)
}

/** The states that have cards, with how many, in the order of the filters. */
export function stateCounts(cards: readonly { support: Unvalidated }[]): Array<[Unvalidated, number]> {
  return UNVALIDATED.flatMap((state) => {
    const count = cards.filter((card) => card.support === state).length
    return count === 0 ? [] : [[state, count] as [Unvalidated, number]]
  })
}

const flat = (text: string): string =>
  text
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line !== '')
    .join(' / ')

/**
 * The words of `reason` in the text of its face: spans count Unicode scalar values, like the text is indexed by here.
 * Lines of the card join with " / ". Null when the text is not known or the span holds nothing.
 */
export function quoteOf(reason: Reason, faces: readonly string[] | undefined): string | null {
  const text = faces?.[reason.face]
  if (text === undefined) return null
  const quote = flat(Array.from(text).slice(reason.span.start, reason.span.end).join(''))
  return quote === '' ? null : quote
}

/** One unread clause: the words of the card it covers (when known) and why the game leaves it to the table. */
export interface ClauseLine {
  key: string
  quote: string | null
  text: string
}

/**
 * The clauses of a card to show or copy. A card with none is left whole to the table, so its text is the quote and
 * the state's words the reason.
 */
export function clauseLines(card: FlaggedCard, faces: readonly string[] | undefined): ClauseLine[] {
  if (card.reasons.length === 0) {
    const whole = flat((faces ?? []).join('\n'))
    const words = NO_CLAUSE[card.support]
    return [
      {
        key: 'whole',
        quote: whole === '' ? null : whole,
        text: words.charAt(0).toUpperCase() + words.slice(1),
      },
    ]
  }
  const lines = reasonLines(card.reasons)
  return card.reasons.map((reason, index) => ({
    key: lines[index]?.key ?? String(index),
    quote: quoteOf(reason, faces),
    text: (lines[index]?.text ?? reason.text).replace(/\.$/, ''),
  }))
}

/** The oracle texts read so far, by oracle id: one string per face. */
export type FaceTexts = ReadonlyMap<string, readonly string[]>

/**
 * The cards as plain text, one block each, blocks apart by an empty line:
 * `Card Name (Manual) - decks: A, B`, then one `  - "<span>": <reason>` line per clause (`  - <reason>` while the words
 * are not known). A card with no deck has no `- decks:` part.
 */
export function copyText(cards: readonly FlaggedCard[], texts: FaceTexts): string {
  return cards
    .map((card) => {
      const decks =
        card.decks.length === 0 ? '' : ` - decks: ${card.decks.map((deck) => deck.name).join(', ')}`
      const lines = clauseLines(card, texts.get(card.oracleId)).map((line) =>
        line.quote === null ? `  - ${line.text}` : `  - "${line.quote}": ${line.text}`,
      )
      return [`${card.name} (${SUPPORT_LABEL[card.support]})${decks}`, ...lines].join('\n')
    })
    .join('\n\n')
}
