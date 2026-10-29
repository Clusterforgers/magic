// Goal: the commanders' colour identity is the union of the cards' identities in WUBRG order, a card lies within it exactly when none of its colours is missing, and the words and the search letters follow.
import { describe, expect, it } from 'vitest'
import type { CardColor } from '@/protocol/generated'
import { cardHit } from '@/features/decks/fakeDecks'
import { identityOf, identityWords, outsideOf, outsideWords, searchLetters } from './identity'

const card = (name: string, identity: CardColor[]) => cardHit(name, { color_identity: identity })

// The identity of a card comes from the server (Scryfall's): a hybrid or Phyrexian symbol counts its colours, devoid
// counts those of the cost, and the commander's own rules text counts, so the cases name what the server sends.
const COMMANDERS: ReadonlyArray<[string, CardColor[][], string[]]> = [
  ['one colour', [['R']], ['R']],
  ['two colours in any order', [['G', 'W']], ['W', 'G']],
  ['a hybrid symbol: both colours', [['G', 'W']], ['W', 'G']],
  ['a Phyrexian symbol: its colour', [['B']], ['B']],
  ['a devoid commander: the colours of its cost', [['U', 'R']], ['U', 'R']],
  ['a colourless commander', [[]], []],
  [
    'a partner pair together',
    [
      ['W', 'B'],
      ['G', 'U'],
    ],
    ['W', 'U', 'B', 'G'],
  ],
  [
    'a partner pair that overlap',
    [
      ['W', 'B'],
      ['B', 'R'],
    ],
    ['W', 'B', 'R'],
  ],
  ['a creature and its Background', [['R'], ['G']], ['R', 'G']],
  ['a colourless partner and a coloured one', [[], ['U']], ['U']],
  ['five colours', [['G', 'W', 'U', 'B', 'R']], ['W', 'U', 'B', 'R', 'G']],
]

describe('identityOf', () => {
  it.each(COMMANDERS)('%s', (_name, cards, expected) => {
    expect(identityOf(cards.map((identity, index) => card(`Commander ${index}`, identity)))).toEqual(expected)
  })

  it('is empty for no cards', () => {
    expect(identityOf([])).toEqual([])
  })
})

describe('outsideOf', () => {
  it.each<[string, CardColor[], string[], string[]]>([
    ['a colourless card is within any identity', [], ['W'], []],
    ['a card of the identity', ['W'], ['W', 'U'], []],
    ['a card with every colour of a two-colour identity', ['W', 'U'], ['W', 'U'], []],
    ['a card with one colour missing', ['W', 'R'], ['W', 'U'], ['R']],
    ['a card with several colours missing, in WUBRG order', ['G', 'B', 'R'], ['W'], ['B', 'R', 'G']],
    ['any colour is outside a colourless commander', ['U'], [], ['U']],
    ['a colourless card is within a colourless commander', [], [], []],
  ])('%s', (_name, cardIdentity, commander, expected) => {
    expect(outsideOf(card('Card', cardIdentity), commander)).toEqual(expected)
  })
})

describe('the words', () => {
  it.each<[string[], string]>([
    [[], 'Colorless'],
    [['G'], 'Green'],
    [['W', 'U'], 'White and Blue'],
    [['W', 'B', 'G'], 'White, Black and Green'],
  ])('names %j as %s', (identity, words) => {
    expect(identityWords(identity)).toBe(words)
  })

  it('asks the search for the identity, or colourless cards for a colourless commander', () => {
    expect(searchLetters(['U', 'R'])).toEqual(['U', 'R'])
    expect(searchLetters([])).toEqual(['C'])
  })

  it('names the colours outside', () => {
    expect(outsideWords(['R', 'G'])).toBe('Outside identity: Red, Green')
  })
})
