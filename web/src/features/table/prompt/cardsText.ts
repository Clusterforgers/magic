/** The words and rules of the prompts that choose cards from a list and put cards in order. */
import type { ObjectId, ObjectView } from '@/protocol/generated'
import { typeLine } from '../cardModel'
import { numberWord } from './choiceText'
import { countOf } from './combatText'
import { ordinal } from './orderText'

/** From this many cards on offer a list gets a filter box. */
export const FILTER_FROM = 12

/** The cards on offer, each once, in the order offered. */
export function distinctCards(from: readonly ObjectId[]): ObjectId[] {
  return [...new Set(from)]
}

/** The fewest and most cards that may be chosen. */
export interface Bounds {
  lo: number
  hi: number
}

/** What the engine asked for, held to the `total` cards on offer, so a prompt can never ask for more than it shows. */
export function boundsOf(min: number, max: number, total: number): Bounds {
  const hi = Math.min(max, total)
  return { lo: Math.min(min, hi), hi }
}

/** Whether `count` cards may be confirmed. */
export function countFits(count: number, { lo, hi }: Bounds): boolean {
  return count >= lo && count <= hi
}

/**
 * The cards chosen after a press on `id`: a chosen card is taken back; with room for one a press on another card
 * changes the choice; with room for several a full choice stays as it is (the very same list). Order of choosing is kept.
 */
export function pressedCard(chosen: readonly ObjectId[], id: ObjectId, hi: number): readonly ObjectId[] {
  if (chosen.includes(id)) return chosen.filter((card) => card !== id)
  if (hi === 1) return [id]
  return chosen.length < hi ? [...chosen, id] : chosen
}

const cardWord = (n: number): string => (n === 1 ? 'card' : 'cards')

/** The sentence for a choice that came without words of its own, from the fewest and most. */
export function cardsTitle({ lo, hi }: Bounds, total: number): string {
  if (total === 0) return 'There is nothing to choose.'
  if (hi === 0) return 'No card can be chosen.'
  if (lo === hi) return hi === 1 ? 'Choose a card.' : `Choose ${numberWord(hi)} cards.`
  if (lo === 0 && hi === total) return 'Choose any number of the cards.'
  if (lo === 0) return `Choose up to ${numberWord(hi)} ${cardWord(hi)}.`
  if (hi === total) return `Choose at least ${numberWord(lo)} ${cardWord(lo)}.`
  return `Choose ${numberWord(lo)} to ${numberWord(hi)} cards.`
}

/** The count line: what is asked in numbers, "Choose 1 of 3". */
export function cardsRule({ lo, hi }: Bounds, total: number): string {
  if (total === 0) return 'No cards to choose from'
  if (hi === 0) return 'No card can be chosen'
  if (lo === hi) return `Choose ${hi} of ${total}`
  if (lo === 0 && hi === total) return `Choose any number of ${total}`
  if (lo === 0) return `Choose up to ${hi} of ${total}`
  if (hi === total) return `Choose at least ${lo} of ${total}`
  return `Choose ${lo} to ${hi} of ${total}`
}

/** The keys of the card prompt for a screen reader; `count` is the number of cards in the list. */
export function cardKeysWords(count: number): string {
  if (count === 0) return ''
  if (count === 1) return 'Press 1 to choose it or take it back.'
  if (count <= 9) return `Press 1 to ${count} to choose a card or take it back.`
  return 'Press 1 to 9, or 0, to choose one of the first ten or take it back.'
}

/** The sentence before anything is chosen: who asks and the card's words, or the generic sentence. */
export function cardsAsk(asker: string, title: string): string {
  return `${asker}: ${title}`
}

/**
 * What only a screen reader hears after that sentence: the numbers when the card's words did not say them (`rule` is
 * null when the title is the generic sentence, which does) and the keys.
 */
export function cardsSpoken(rule: string | null, keys: string): string {
  return [rule === null ? '' : `${rule}.`, keys].filter((part) => part !== '').join(' ')
}

/** The sentence once something is chosen: how many, and what is left to do. */
export function cardsProgress(asker: string, chosen: number, { lo, hi }: Bounds): string {
  if (hi === 0) return `${asker}: nothing can be chosen. Press Confirm to go on.`
  if (chosen < lo) return `${asker}: ${chosen} of ${lo} chosen. Choose ${lo - chosen} more.`
  const head = `${asker}: ${countOf(chosen, 'card')} chosen.`
  if (hi === 1) return `${head} Press Confirm, or choose another card instead.`
  if (chosen >= hi) return `${head} Press Confirm, or take one back.`
  return `${head} Press Confirm, or choose up to ${hi - chosen} more.`
}

/** Whether a card passes the filter: every word of it is in the card's name or type line. A card the view does not hold passes only an empty filter. */
export function matchesFilter(object: ObjectView | undefined, filter: string): boolean {
  const words = filter
    .toLowerCase()
    .split(/\s+/u)
    .filter((word) => word !== '')
  if (words.length === 0) return true
  if (object === undefined) return false
  const text = `${object.name} ${typeLine(object)}`.toLowerCase()
  return words.every((word) => text.includes(word))
}

/** The title of an order of cards that came without words of its own. */
export function orderTitle(count: number): string {
  return count === 1 ? 'Put the card in order.' : `Put the ${count} cards in order.`
}

/** The sentence above the list: the card's words, then where the card moved last went. */
export function orderSentence(asker: string, title: string, note: string | null): string {
  const head = `${asker}: ${title}`
  return note === null ? head : `${head} ${note}`
}

/** Where a card stands in the list, for the sentence that follows a move: "now 2nd of 3", "now 1st of 3, first". */
export function cardPlaceWords(place: number, count: number): string {
  const tail = place === 0 ? ', first' : place === count - 1 ? ', last' : ''
  return `now ${ordinal(place + 1)} of ${count}${tail}`
}

/** What only a screen reader hears after the sentence of the order prompt: where the list starts and the keys. */
export const ORDER_KEYS = 'The top card is first. Alt with the up or down arrow moves the focused card.'
