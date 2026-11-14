// Goal: a choice of cards under a total closes the cards that would pass it, counts a negative power down, and words the total so far.
import { describe, expect, it } from 'vitest'
import type { CardsTotal } from '@/protocol/generated'
import { cardsTotalWords, chosenSum, fitsCardsTotal, overTotal } from './cardsTotal'

const from = [1, 2, 3, 4]
const total: CardsTotal = { most: 4, values: [3, 2, 1, -1], stat: 'power' }

describe('cards under a total', () => {
  it.each([
    [[], [], 0],
    [[1], [2], 3],
    [[1, 4], [], 2],
    [[2, 3], [1], 3],
  ])('chosen %j closes %j and sums %i', (chosen, shut, sum) => {
    expect([...overTotal(from, total, chosen)]).toEqual(shut)
    expect(chosenSum(from, total, chosen)).toBe(sum)
  })

  it('fits only within the most, and anything without a total', () => {
    expect(fitsCardsTotal(from, total, [1, 3])).toBe(true)
    expect(fitsCardsTotal(from, total, [1, 2])).toBe(false)
    expect(fitsCardsTotal(from, undefined, [1, 2])).toBe(true)
    expect(overTotal(from, undefined, []).size).toBe(0)
  })

  it('words the total so far', () => {
    expect(cardsTotalWords(from, total, [1])).toBe('Total power 3 of at most 4.')
    expect(cardsTotalWords(from, null, [1])).toBe('')
  })
})
