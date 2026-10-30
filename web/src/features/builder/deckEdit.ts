/**
 * The deck as the builder edits it: lines of copies of a card in a zone, changed by pure functions that return a new
 * draft (the same draft when nothing changes). A card has one line per zone.
 */
import type { CardHit, CheckRequest, DeckBody, DeckLineBody, DeckView, DeckZone } from '@/protocol/generated'

/** One line of the draft: `count` copies of `card` in `zone`, in the art of `printingId` (null for the default). */
export interface Line {
  zone: DeckZone
  card: CardHit
  count: number
  printingId: string | null
}

/** What the builder edits: the fields of a save, with the cards as lines that carry the whole card. */
export interface Draft {
  name: string
  format: string
  notes: string
  lines: readonly Line[]
}

/** Most cards in a deck, copies counted (`MAX_DECK_CARDS` in `crates/mtg-protocol/src/rest/decks.rs`). */
export const MAX_DECK_CARDS = 250

const sameLine = (line: Line, zone: DeckZone, oracleId: string): boolean =>
  line.zone === zone && line.card.oracle_id === oracleId

/** The line of `oracleId` in `zone`, if there is one. */
export const lineIn = (draft: Draft, zone: DeckZone, oracleId: string): Line | undefined =>
  draft.lines.find((line) => sameLine(line, zone, oracleId))

/** Copies in `zone`, or in the whole deck when no zone is given. */
export const totalOf = (draft: Draft, zone?: DeckZone): number =>
  draft.lines.reduce((sum, line) => (zone === undefined || line.zone === zone ? sum + line.count : sum), 0)

/** The pane a phone opens on: the deck with its report when it has cards, the search when it is empty. */
export const openingPane = (draft: Draft): 'search' | 'deck' => (totalOf(draft) > 0 ? 'deck' : 'search')

/** Copies of one card in `zone`, or in the whole deck when no zone is given. */
export const copiesOf = (draft: Draft, oracleId: string, zone?: DeckZone): number =>
  draft.lines.reduce(
    (sum, line) =>
      line.card.oracle_id === oracleId && (zone === undefined || line.zone === zone) ? sum + line.count : sum,
    0,
  )

/** True while another copy fits under the card limit of a deck. */
export const hasRoom = (draft: Draft): boolean => totalOf(draft) < MAX_DECK_CARDS

/** The lines of `zone` by card name, then by oracle id, so the order never depends on how the deck was built. */
export const linesIn = (draft: Draft, zone: DeckZone): Line[] =>
  draft.lines
    .filter((line) => line.zone === zone)
    .sort(
      (a, b) =>
        a.card.name.localeCompare(b.card.name, 'en', { sensitivity: 'base' }) ||
        a.card.oracle_id.localeCompare(b.card.oracle_id),
    )

/**
 * The draft of a stored deck. Lines of one card in one zone become one line, with the printing of the first line that
 * names one, so a card never has two rows to edit.
 */
export function draftOf(view: Pick<DeckView, 'name' | 'format' | 'notes' | 'cards'>): Draft {
  const lines: Line[] = []
  for (const stored of view.cards) {
    const existing = lines.findIndex((line) => sameLine(line, stored.zone, stored.card.oracle_id))
    const line = lines[existing]
    if (line === undefined) {
      lines.push({
        zone: stored.zone,
        card: stored.card,
        count: stored.count,
        printingId: stored.printing_id,
      })
    } else {
      lines[existing] = {
        ...line,
        count: line.count + stored.count,
        printingId: line.printingId ?? stored.printing_id,
      }
    }
  }
  return { name: view.name, format: view.format, notes: view.notes, lines }
}

const bodyLines = (lines: readonly Line[]): DeckLineBody[] =>
  lines.map((line) => ({
    zone: line.zone,
    oracle_id: line.card.oracle_id,
    printing_id: line.printingId,
    count: line.count,
  }))

/** The body of `PUT /api/decks/{id}` for a draft: the name trimmed, the lines in the draft's order. */
export const bodyOf = (draft: Draft): DeckBody => ({
  name: draft.name.trim(),
  format: draft.format,
  notes: draft.notes,
  cards: bodyLines(draft.lines),
})

/** The body of `POST /api/decks/check` for a draft: the unsaved list a save would check, without name or notes. */
export const checkRequestOf = ({ format, lines }: Pick<Draft, 'format' | 'lines'>): CheckRequest => ({
  deck: null,
  format,
  cards: bodyLines(lines),
})

const key = (line: Line): string =>
  `${line.zone}|${line.card.oracle_id}|${line.printingId ?? ''}|${line.count}`

/**
 * What the deck check sees of a draft, as text: the format and the lines in any order. Drafts with equal keys have
 * equal reports; the name and the notes are not part of it.
 */
export const checkKey = ({ format, lines }: Pick<Draft, 'format' | 'lines'>): string =>
  [format, ...lines.map(key).sort()].join('\n')

/** True when two drafts would save the same deck: equal fields and the same lines, in any order. */
export function sameDeck(a: Draft, b: Draft): boolean {
  if (a.name !== b.name || a.format !== b.format || a.notes !== b.notes) return false
  if (a.lines.length !== b.lines.length) return false
  const left = a.lines.map(key).sort()
  const right = b.lines.map(key).sort()
  return left.every((entry, index) => entry === right[index])
}

/** Sets the copies of a line; zero or fewer removes it, and more than the deck has room for is cut back. */
export function setCount(draft: Draft, zone: DeckZone, oracleId: string, count: number): Draft {
  const line = lineIn(draft, zone, oracleId)
  if (line === undefined) return draft
  if (count <= 0) return removeLine(draft, zone, oracleId)
  const room = MAX_DECK_CARDS - (totalOf(draft) - line.count)
  const next = Math.min(Math.floor(count), room)
  if (next === line.count || next <= 0) return draft
  return { ...draft, lines: draft.lines.map((item) => (item === line ? { ...item, count: next } : item)) }
}

/** Adds `count` copies of `card` to `zone`: onto its line there, or as a new line at the end. */
export function addCard(draft: Draft, card: CardHit, zone: DeckZone, count = 1): Draft {
  const copies = Math.min(Math.floor(count), MAX_DECK_CARDS - totalOf(draft))
  if (!(copies > 0)) return draft
  const line = lineIn(draft, zone, card.oracle_id)
  if (line === undefined) {
    return { ...draft, lines: [...draft.lines, { zone, card, count: copies, printingId: null }] }
  }
  return {
    ...draft,
    lines: draft.lines.map((item) => (item === line ? { ...item, count: item.count + copies } : item)),
  }
}

/** One more copy of a card already in `zone`. */
export function increment(draft: Draft, zone: DeckZone, oracleId: string): Draft {
  const line = lineIn(draft, zone, oracleId)
  return line === undefined ? draft : setCount(draft, zone, oracleId, line.count + 1)
}

/** One copy fewer; the last copy removes the line. */
export function decrement(draft: Draft, zone: DeckZone, oracleId: string): Draft {
  const line = lineIn(draft, zone, oracleId)
  return line === undefined ? draft : setCount(draft, zone, oracleId, line.count - 1)
}

/** Takes a card out of `zone` whatever its count. */
export function removeLine(draft: Draft, zone: DeckZone, oracleId: string): Draft {
  const line = lineIn(draft, zone, oracleId)
  return line === undefined ? draft : { ...draft, lines: draft.lines.filter((item) => item !== line) }
}

/**
 * Moves `count` copies (all of them by default) of a card from `zone` to `to`, onto the card's line there if it has
 * one. A line that keeps copies stays where it is; a new line keeps the printing of the one it came from.
 */
export function moveLine(
  draft: Draft,
  zone: DeckZone,
  oracleId: string,
  to: DeckZone,
  count?: number,
): Draft {
  const line = lineIn(draft, zone, oracleId)
  if (line === undefined || zone === to) return draft
  const moved = Math.min(Math.floor(count ?? line.count), line.count)
  if (!(moved > 0)) return draft
  const target = lineIn(draft, to, oracleId)
  const lines: Line[] = []
  for (const item of draft.lines) {
    if (item === line) {
      if (moved < line.count) lines.push({ ...line, count: line.count - moved })
    } else if (item === target) {
      lines.push({ ...item, count: item.count + moved })
    } else {
      lines.push(item)
    }
  }
  if (target === undefined) lines.push({ ...line, zone: to, count: moved })
  return { ...draft, lines }
}
