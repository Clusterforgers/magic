// Goal: the untapped mana sources of a seat are counted per colour from the permanents it controls and may tap now: a dual land counts for both its colours, a tapped, opposing or summoning-sick source for none.
import { describe, expect, it } from 'vitest'
import type { ObjectView } from '@/protocol/generated'
import { card, forest } from '@/test-utils/cards'
import { sourceCounts } from './untappedSources'

type Colour = 'White' | 'Blue' | 'Black' | 'Red' | 'Green' | 'Colorless'
const land = (produces: Colour, over: Partial<ObjectView> = {}) =>
  forest({ abilities: [{ Mana: { produces } }], ...over })
const tapped = { tapped: true, damage: 0, summoning_sick: false }

// Counts are in the order White, Blue, Black, Red, Green, Colorless.
describe('sourceCounts', () => {
  it.each<[string, ObjectView[], number[]]>([
    ['nothing', [], [0, 0, 0, 0, 0, 0]],
    ['two forests and a wastes', [land('Green'), land('Green'), land('Colorless')], [0, 0, 0, 0, 2, 1]],
    ['a tapped land', [land('Red', { permanent: tapped })], [0, 0, 0, 0, 0, 0]],
    ['an opponent land', [land('Red', { controller: 1 })], [0, 0, 0, 0, 0, 0]],
    [
      'a dual land counts for both',
      [forest({ abilities: [{ Mana: { produces: 'White' } }, { Mana: { produces: 'Blue' } }] })],
      [1, 1, 0, 0, 0, 0],
    ],
    [
      'a sick creature',
      [
        card({
          abilities: [{ Mana: { produces: 'Green' } }],
          permanent: { tapped: false, damage: 0, summoning_sick: true },
        }),
      ],
      [0, 0, 0, 0, 0, 0],
    ],
  ])('%s', (_, objects, expected) => {
    expect(sourceCounts(objects, 0)).toEqual(expected)
  })
})
