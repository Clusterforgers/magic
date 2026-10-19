// Goal: for any number of triggers the list on screen starts in the order the engine would take, the answer is always every trigger once with the one on top of the list last, a move changes only the two places it swaps and does nothing at an end, labels tell triggers apart and stay with their trigger, and the ordinals read right.
import { describe, expect, it } from 'vitest'
import type { TriggerEntry } from '@/protocol/generated'
import {
  answerOf,
  defaultOrder,
  isDefaultOrder,
  moved,
  orderSentence,
  ordinal,
  placeWords,
  triggerLabels,
} from './orderText'

const entry = (id: number, ability = 0, when?: string): TriggerEntry => ({
  source: { id, counter: 0 },
  ability,
  ...(when === undefined ? {} : { when }),
})

describe('the two orders', () => {
  it.each([2, 3, 4, 7])(
    'with %i triggers the list starts with the last to trigger and the answer is the order they triggered',
    (count) => {
      const shown = defaultOrder(count)
      expect(shown).toEqual(Array.from({ length: count }, (_, i) => count - 1 - i))
      expect(answerOf(shown)).toEqual(Array.from({ length: count }, (_, i) => i))
      expect(isDefaultOrder(shown)).toBe(true)
    },
  )

  it('answers the list reversed, whatever order it is in, with every trigger once', () => {
    expect(answerOf([2, 0, 1])).toEqual([1, 0, 2])
    expect(answerOf([])).toEqual([])
  })
})

describe('moved', () => {
  it.each([
    [[2, 1, 0], 1, -1, [1, 2, 0]],
    [[2, 1, 0], 1, 1, [2, 0, 1]],
    [[2, 1, 0], 0, 1, [1, 2, 0]],
    [[2, 1, 0], 2, -1, [2, 0, 1]],
    [[2, 1, 0], 0, -1, [2, 1, 0]],
    [[2, 1, 0], 2, 1, [2, 1, 0]],
    [[0], 0, 1, [0]],
  ] as const)('%j at place %i by %i gives %j', (shown, place, by, expected) => {
    expect(moved(shown, place, by)).toEqual(expected)
  })

  it('does not change the list it is given', () => {
    const shown = [2, 1, 0]
    moved(shown, 0, 1)
    expect(shown).toEqual([2, 1, 0])
  })

  it('walks every trigger from one end to the other and stays a permutation', () => {
    let shown: number[] = defaultOrder(5)
    for (let i = 0; i < 4; i += 1) shown = moved(shown, i, 1)
    expect(shown).toEqual([3, 2, 1, 0, 4])
    expect([...shown].sort()).toEqual([0, 1, 2, 3, 4])
    expect(isDefaultOrder(shown)).toBe(false)
  })
})

describe('triggerLabels', () => {
  it('names the source and what it waits for', () => {
    expect(
      triggerLabels(
        [entry(1, 0, 'When it enters the battlefield'), entry(2, 1, 'Whenever you draw a card')],
        ['Mulldrifter', 'Rhystic Study'],
      ),
    ).toEqual(['Mulldrifter: When it enters the battlefield', 'Rhystic Study: Whenever you draw a card'])
  })

  it('falls back to the ability number without words and to "A card" without the source', () => {
    expect(triggerLabels([entry(1, 2), entry(2, 0, '')], [undefined, 'Bear'])).toEqual([
      'A card: ability 3',
      'Bear: ability 1',
    ])
  })

  it('numbers triggers that read the same in the order they triggered', () => {
    const entries = [
      entry(1, 0, 'When it dies'),
      entry(2, 0, 'When it dies'),
      entry(3, 0, 'When it dies'),
      entry(4, 0, 'At the beginning of your upkeep'),
    ]
    expect(triggerLabels(entries, ['Elf', 'Elf', 'Elf', 'Elf'])).toEqual([
      'Elf: When it dies (1)',
      'Elf: When it dies (2)',
      'Elf: When it dies (3)',
      'Elf: At the beginning of your upkeep',
    ])
  })
})

describe('words', () => {
  it.each([
    [1, '1st'],
    [2, '2nd'],
    [3, '3rd'],
    [4, '4th'],
    [10, '10th'],
    [11, '11th'],
    [12, '12th'],
    [13, '13th'],
    [21, '21st'],
    [22, '22nd'],
    [23, '23rd'],
    [101, '101st'],
    [111, '111th'],
  ])('%i is the %s', (n, words) => {
    expect(ordinal(n)).toBe(words)
  })

  it.each([
    [0, 3, 'now 1st of 3, resolves first'],
    [1, 3, 'now 2nd of 3'],
    [2, 3, 'now 3rd of 3, resolves last'],
    [3, 5, 'now 4th of 5'],
  ])('place %i of %i reads "%s"', (place, count, words) => {
    expect(placeWords(place, count)).toBe(words)
  })

  it('asks for the order, and says where the last move put a trigger', () => {
    expect(orderSentence(2, null)).toBe(
      'Order your 2 triggers: the top one resolves first. Confirm when the order is right.',
    )
    expect(orderSentence(3, 'Elf is now 2nd of 3.')).toBe(
      'Order your 3 triggers: the top one resolves first. Elf is now 2nd of 3.',
    )
  })
})
