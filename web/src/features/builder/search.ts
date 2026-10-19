/** The filters of the card search: their values, what is wrong with a mana value, and how they become a request. */
import { DECK_FORMATS } from '@/features/decks/format'
import type { CardQuery } from '@/net/cards'

/** The colour identities a card can have, in the order the search writes them: WUBRG, then colourless. */
export const IDENTITY_LETTERS: readonly string[] = ['W', 'U', 'B', 'R', 'G', 'C']

/** The name of each identity letter. */
export const IDENTITY_NAME: Readonly<Record<string, string>> = {
  W: 'White',
  U: 'Blue',
  B: 'Black',
  R: 'Red',
  G: 'Green',
  C: 'Colorless',
}

/** The formats the search can narrow to: the deck formats that Scryfall keeps legality for (Freeform has none). */
export const FILTER_FORMATS: readonly string[] = DECK_FORMATS.filter((key) => key !== 'freeform')

/** What the search box and the filters hold. */
export interface SearchFilters {
  text: string
  /** The identity letters that are on, in `IDENTITY_LETTERS` order. */
  identity: readonly string[]
  type: string
  manaValue: string
  /** A format key, or empty for any. */
  format: string
}

/** No text and no filter. */
export const NO_FILTERS: SearchFilters = { text: '', identity: [], type: '', manaValue: '', format: '' }

const MANA_VALUE = /^(\d{1,2})(?:-(\d{1,2})|\+)?$/

/** What is wrong with a mana value, in words, or null when it is empty or the server will take it. */
export function manaValueProblem(value: string): string | null {
  const text = value.trim()
  if (text === '') return null
  const match = MANA_VALUE.exec(text)
  if (match === null) return 'Use a number, a range such as 2-4, or a minimum such as 5+.'
  const [, low, high] = match
  if (high !== undefined && Number(low) > Number(high))
    return 'A range runs from the smaller number to the larger, such as 2-4.'
  return null
}

/** `identity` with `letter` switched on or off, kept in `IDENTITY_LETTERS` order. */
export function toggleLetter(identity: readonly string[], letter: string): string[] {
  const on = new Set(identity)
  if (on.has(letter)) on.delete(letter)
  else on.add(letter)
  return IDENTITY_LETTERS.filter((item) => on.has(item))
}

/** True when anything narrows the search: text or any filter (a mana value that is wrong does not count). */
export const hasCriteria = (filters: SearchFilters): boolean => queryOf(filters) !== null

/** The request for `filters` at `page`, or null when there is nothing to search for. A wrong mana value is left out. */
export function queryOf(filters: SearchFilters, page = 1): CardQuery | null {
  const text = filters.text.trim()
  const identity = IDENTITY_LETTERS.filter((letter) => filters.identity.includes(letter)).join('')
  const type = filters.type.trim()
  const manaValue = manaValueProblem(filters.manaValue) === null ? filters.manaValue.trim() : ''
  const format = filters.format.trim()
  if (text === '' && identity === '' && type === '' && manaValue === '' && format === '') return null
  return { text, identity, type, manaValue, format, page }
}
