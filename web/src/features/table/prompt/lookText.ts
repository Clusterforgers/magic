/** The rules and words of the prompts that look at the top of a library: a scry or surveil split and the order of the cards put back. */
import type { ObjectId, Zone } from '@/protocol/generated'
import { countOf } from './combatText'

/** Where the cards a split sends away go: the bottom of the library (scry) or the graveyard (surveil). */
export type Away = 'bottom' | 'graveyard'

/** The engine's question about the cards looked at, as `ChooseCards` carries it. */
interface LookQuestion {
  min: number
  max: number
  text?: string | undefined
}

const inLibrary = (zone: Zone | undefined): boolean => typeof zone === 'object' && 'Library' in zone

/**
 * Whether a choice of cards is the split of a scry (the cards looked at go to the bottom) or a surveil (to the
 * graveyard): any number of cards of a library, none to all of them, in the words of the engine's question. Anything
 * else, a search or a discard, is a plain choice of cards. `zones` holds the zone of each of the `total` cards on
 * offer; a card the view does not hold rules the split out.
 */
export function awayOf(
  { min, max, text }: LookQuestion,
  total: number,
  zones: readonly (Zone | undefined)[],
): Away | null {
  if (total < 1 || min !== 0 || max < total || zones.length !== total || !zones.every(inLibrary)) return null
  const words = text ?? ''
  if (/bottom of your library/iu.test(words)) return 'bottom'
  if (/into your graveyard/iu.test(words)) return 'graveyard'
  return null
}

/** The cards that stay on top and those sent away, each in the order looked at (the library's, top first). */
export function splitLooked(
  looked: readonly ObjectId[],
  away: ReadonlySet<ObjectId>,
): { top: ObjectId[]; away: ObjectId[] } {
  return { top: looked.filter((id) => !away.has(id)), away: looked.filter((id) => away.has(id)) }
}

/** The heading of the pile that stays. */
export const TOP_PILE = 'On top'

/** The heading of the pile that goes, by where it goes. */
export function awayPile(away: Away): string {
  return away === 'bottom' ? 'On the bottom' : 'In the graveyard'
}

/** The sentence of the split when the card gave no words. */
export function lookTitle(away: Away, total: number): string {
  const place = away === 'bottom' ? 'on the bottom of your library' : 'into your graveyard'
  return total === 1 ? `Put the card ${place}, or keep it on top.` : `Choose which of the cards go ${place}.`
}

/** What the sentence says once the player moved something: how many stay and how many go. */
export function lookProgress(asker: string, top: number, away: number, kind: Away): string {
  const place = kind === 'bottom' ? 'the bottom' : 'the graveyard'
  const stay = top === 0 ? 'None stay on top' : `${countOf(top, 'card')} stay${top === 1 ? 's' : ''} on top`
  const go = away === 0 ? 'none go' : `${away} go${away === 1 ? 'es' : ''}`
  return `${asker}: ${stay}, ${go} to ${place}. Press Confirm when it is right.`
}

/** The keys of the split for a screen reader; `count` cards are numbered in the order looked at, the top card first. */
export function lookKeys(count: number): string {
  if (count <= 1) return 'Press 1 to move the card, then Confirm.'
  const range = count <= 9 ? `1 to ${count}` : '1 to 9, or 0,'
  return `Press ${range} to move that card between the top and the other pile, the top card being 1.`
}

/** The result of the split in one line, both piles in the order looked at: "On top: A, B. On the bottom: C." */
export function lookPreview(top: readonly string[], away: readonly string[], kind: Away): string {
  const list = (names: readonly string[]): string => (names.length === 0 ? 'none' : names.join(', '))
  return `${TOP_PILE}: ${list(top)}. ${awayPile(kind)}: ${list(away)}.`
}

/** Which end of the library the cards of an order go to, from the engine's words ("Put the cards on the bottom ..."). */
export function pileOf(text: string): 'top' | 'bottom' {
  return /\bbottom\b/iu.test(text) ? 'bottom' : 'top'
}

/** The heading over the strip of cards being put in order. */
export function pileHeading(pile: 'top' | 'bottom'): string {
  return pile === 'top' ? 'Going on top of your library' : 'Going on the bottom of your library'
}

/** The result of the order in one line, from the top down: "Top of your library: A, then B, then C." */
export function orderPreview(names: readonly string[], pile: 'top' | 'bottom'): string {
  const head = pile === 'top' ? 'Top of your library' : 'Bottom of your library, top to bottom'
  return `${head}: ${names.join(', then ')}.`
}
