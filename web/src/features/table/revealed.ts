/** Pure words and lookups for what Reveal and Note put in the view: a seat's revealed hand cards and top card, and a card's note. */
import type { ObjectId, PlayerView } from '@/protocol/generated'
import { cardCount } from './podFigures'

/** Where a revealed card is: in its owner's hand, or on top of their library. */
export type RevealedPlace = 'hand' | 'top'

/** The cards a seat showed to every seat by hand, or that a rule keeps revealed. */
export interface RevealedCards {
  /** The cards of the hand, in the hand's order. */
  hand: readonly ObjectId[]
  /** The top card of the library, if shown. */
  top: ObjectId | undefined
  /** All of them: the hand cards, then the top card. */
  all: readonly ObjectId[]
}

const NONE: RevealedCards = { hand: [], top: undefined, all: [] }

/** What `player` shows: its revealed hand cards and its revealed top card. */
export function revealedOf(player: PlayerView | undefined): RevealedCards {
  const hand = player?.revealed_hand ?? []
  const top = player?.revealed_top
  if (hand.length === 0 && top === undefined) return NONE
  return { hand, top, all: top === undefined ? hand : [...hand, top] }
}

/** Where `id` is shown to every seat, or undefined when no seat has revealed it. */
export function revealedPlace(players: readonly PlayerView[], id: ObjectId): RevealedPlace | undefined {
  for (const player of players) {
    if (player.revealed_hand?.includes(id) === true) return 'hand'
    if (player.revealed_top === id) return 'top'
  }
  return undefined
}

/** "Revealed": the mark's word. */
export const REVEALED_WORD = 'Revealed'

/** What a seat shows, as a chip names it: "Revealed 3 cards". */
export function revealedLabel(count: number): string {
  return `${REVEALED_WORD} ${cardCount(count)}`
}

/** Where the revealed cards are: "2 in hand, the top of the library". */
export function revealedWhere(cards: RevealedCards): string {
  return [
    cards.hand.length === 0 ? null : `${cards.hand.length} in hand`,
    cards.top === undefined ? null : 'the top of the library',
  ]
    .filter((part) => part !== null)
    .join(', ')
}

/** The chip's accessible name: "Revealed 3 cards: 2 in hand, the top of the library". */
export function revealedChipName(cards: RevealedCards): string {
  return `${revealedLabel(cards.all.length)}: ${revealedWhere(cards)}`
}

/** What the preview says of a revealed card: "Revealed to every seat, in Ana's hand." */
export function revealedLine(place: RevealedPlace, owner: string): string {
  return place === 'hand'
    ? `${REVEALED_WORD} to every seat, in ${owner}'s hand.`
    : `${REVEALED_WORD} to every seat, on top of ${owner}'s library.`
}

/** How many words of a note the mark on the face shows. */
export const NOTE_WORDS = 3

/** The most characters the mark on the face shows of a note. */
export const NOTE_CHARS = 18

/** The first words of a note for the mark on a face, cut at three words or 18 characters and ended with "…". */
export function noteLead(note: string): string {
  const words = note
    .trim()
    .split(/\s+/)
    .filter((word) => word !== '')
  let lead = words.slice(0, NOTE_WORDS).join(' ')
  let cut = words.length > NOTE_WORDS
  if (lead.length > NOTE_CHARS) {
    lead = lead.slice(0, NOTE_CHARS - 1).trimEnd()
    cut = true
  }
  return cut ? `${lead}…` : lead
}

/** The mark's accessible name, with the whole note: "Note: Back in two turns". */
export function noteName(note: string): string {
  return `Note: ${note}`
}

/** What a card's spoken name adds for its revealed state and note. */
export function handMarkWords(place: RevealedPlace | undefined, note: string | undefined): string[] {
  return [
    place === undefined ? null : 'revealed to every seat',
    note === undefined ? null : `note: ${note}`,
  ].filter((part) => part !== null)
}
