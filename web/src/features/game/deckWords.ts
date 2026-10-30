/**
 * The words for a seat's deck in the lobby: what the whole table is told (name, format and a count of cards played
 * by hand, never the name of a card) and what the owner is told about a deck that was refused. Pure, so the
 * wording is tested without a page.
 */
import { formatLabel } from '@/features/decks/format'
import { cardsByHand } from '@/lib/byHand'
import type { Issue, SeatDeck } from '@/protocol/generated'

/** The format of a seat's deck as people say it: a built-in deck is a Normal deck, the rest keep their names. */
export const deckFormat = (key: string): string => (key === 'normal' ? 'Normal' : formatLabel(key))

/** "Burn, Modern, 3 cards played by hand": everything the table learns of a deck. */
export function deckWords(deck: SeatDeck): string {
  return `${deck.name}, ${deckFormat(deck.format)}, ${cardsByHand(deck.manual_cards)}`
}

/** Whether two decks are the same to the table: the same name, format and count. */
export const sameDeck = (a: SeatDeck, b: SeatDeck): boolean =>
  a.name === b.name && a.format === b.format && a.manual_cards === b.manual_cards

/** What is true of the seat's deck now, as the lead of a sentence about a choice that did not go through. */
export function stillWords(current: SeatDeck | null | undefined): string {
  if (current === undefined) return 'Your deck has not changed.'
  return current === null ? 'You still have no deck.' : `Your deck is still ${deckWords(current)}.`
}

/** One issue of a refused deck as a sentence: the server's own words, ending in a full stop. */
export function issueWords(issue: Pick<Issue, 'message'>): string {
  const text = issue.message.trim()
  if (text === '') return 'The deck breaks a rule of the game.'
  const capital = text.charAt(0).toUpperCase() + text.slice(1)
  return /[.!?]$/.test(capital) ? capital : `${capital}.`
}

/** What to say when a deck was refused and the server named no issue: it holds cards the game cannot play. */
export const NO_ISSUES = 'The deck holds cards that the game cannot play.'
