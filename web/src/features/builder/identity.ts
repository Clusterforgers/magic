/**
 * The colour identity of a Commander deck as the builder reads it: the commanders' identity is the union of their
 * cards' identities, and a card lies within it when none of its colours is missing there. The identity of each card
 * comes from the server (`CardHit.color_identity`, Scryfall's, which counts hybrid and Phyrexian symbols, colour
 * indicators and rules text and ignores devoid), so only the union and the comparison are decided here. Pure.
 */
import type { CardHit, DeckZone } from '@/protocol/generated'
import { IDENTITY_LETTERS, IDENTITY_NAME } from './search'
import type { Line } from './deckEdit'

/** The five colours in WUBRG order, the order the wire and the filters use. */
export const COLOURS: readonly string[] = IDENTITY_LETTERS.filter((letter) => letter !== 'C')

/** The identity letters of `cards` together, in WUBRG order; empty when every card is colourless. */
export function identityOf(cards: readonly Pick<CardHit, 'color_identity'>[]): string[] {
  const all = new Set<string>(cards.flatMap((card) => card.color_identity))
  return COLOURS.filter((letter) => all.has(letter))
}

/** The cards of `zone` in `lines`, one entry per card whatever its copies. */
export const cardsIn = (lines: readonly Line[], zone: DeckZone): CardHit[] =>
  lines.filter((line) => line.zone === zone).map((line) => line.card)

/** The colours of `card` that `identity` lacks, in WUBRG order; empty when the card lies within it. */
export function outsideOf(card: Pick<CardHit, 'color_identity'>, identity: readonly string[]): string[] {
  return COLOURS.filter(
    (letter) => (card.color_identity as readonly string[]).includes(letter) && !identity.includes(letter),
  )
}

/** The identity as words: "White, Blue and Black", "Green", or "Colorless" for none. */
export function identityWords(identity: readonly string[]): string {
  const names = identity.map((letter) => IDENTITY_NAME[letter] ?? letter)
  if (names.length === 0) return 'Colorless'
  if (names.length === 1) return names[0] ?? ''
  return `${names.slice(0, -1).join(', ')} and ${names[names.length - 1] ?? ''}`
}

/**
 * The letters the search sends to show the cards within `identity`: the identity itself, or `C` for a colourless
 * commander, whose deck holds colourless cards only.
 */
export const searchLetters = (identity: readonly string[]): string[] =>
  identity.length === 0 ? ['C'] : [...identity]

/** The words under a card that lies outside the identity: "Outside identity: Red". */
export function outsideWords(outside: readonly string[]): string {
  return `Outside identity: ${outside.map((letter) => IDENTITY_NAME[letter] ?? letter).join(', ')}`
}
