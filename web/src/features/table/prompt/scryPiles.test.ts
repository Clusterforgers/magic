/** Goal: the piles of a scry never lose or repeat a card whatever is sent across, shifted or dropped between them, keep the order of both piles, put a dropped card before the card it is dropped on or last, and read as the scry that changes nothing only while every card is on top in the order looked at, for 1 to 5 cards. */
import { describe, expect, it } from 'vitest'
import { crossed, isStart, pileOfCard, placed, scryPreview, shifted, startPiles } from './scryPiles'

const cards = (count: number): number[] => Array.from({ length: count }, (_, i) => i + 1)

describe('startPiles', () => {
  it.each([1, 2, 3, 4, 5])('puts all %i cards on top in the order looked at', (count) => {
    const piles = startPiles(cards(count))
    expect(piles).toEqual({ top: cards(count), bottom: [] })
    expect(isStart(piles, cards(count))).toBe(true)
  })
})

describe('crossed', () => {
  it.each([1, 2, 3, 4, 5])('sends each of %i cards to the bottom and back, last in its pile', (count) => {
    let piles = startPiles(cards(count))
    for (const id of cards(count)) piles = crossed(piles, id)
    expect(piles).toEqual({ top: [], bottom: cards(count) })
    for (const id of [...cards(count)].reverse()) piles = crossed(piles, id)
    expect(piles).toEqual({ top: [...cards(count)].reverse(), bottom: [] })
    expect(isStart(piles, cards(count))).toBe(count === 1)
  })

  it('leaves a card of neither pile alone', () => {
    const piles = startPiles(cards(3))
    expect(crossed(piles, 9)).toBe(piles)
    expect(pileOfCard(piles, 9)).toBeNull()
  })
})

describe('shifted', () => {
  it('moves a card toward the top or the bottom of its own pile only', () => {
    const piles = { top: [1, 2, 3], bottom: [4, 5] }
    expect(shifted(piles, 3, -1)).toEqual({ top: [1, 3, 2], bottom: [4, 5] })
    expect(shifted(piles, 4, 1)).toEqual({ top: [1, 2, 3], bottom: [5, 4] })
  })

  it('stays put at either end of the pile', () => {
    const piles = { top: [1, 2], bottom: [3] }
    expect(shifted(piles, 1, -1)).toBe(piles)
    expect(shifted(piles, 2, 1)).toBe(piles)
    expect(shifted(piles, 3, 1)).toBe(piles)
  })
})

describe('placed', () => {
  it.each([
    [
      'before a card of the other pile',
      { top: [1, 2, 3], bottom: [4, 5] },
      2,
      'bottom',
      5,
      [1, 3],
      [4, 2, 5],
    ],
    ['last when no card is named', { top: [1, 2, 3], bottom: [4, 5] }, 1, 'bottom', null, [2, 3], [4, 5, 1]],
    ['first in an empty pile', { top: [1, 2], bottom: [] }, 2, 'bottom', null, [1], [2]],
    ['within its own pile, before another', { top: [1, 2, 3], bottom: [] }, 3, 'top', 1, [3, 1, 2], []],
    ['last in its own pile', { top: [1, 2, 3], bottom: [] }, 1, 'top', null, [2, 3, 1], []],
    ['back on top before a card', { top: [1], bottom: [2, 3] }, 3, 'top', 1, [3, 1], [2]],
  ] as const)('%s', (_name, piles, id, to, before, top, bottom) => {
    expect(placed(piles, id, to, before)).toEqual({ top, bottom })
  })

  it('does nothing for a card dropped on itself or one that is not there', () => {
    const piles = { top: [1, 2], bottom: [3] }
    expect(placed(piles, 2, 'top', 2)).toBe(piles)
    expect(placed(piles, 9, 'top', null)).toBe(piles)
  })

  it('puts a card last when the card it is dropped before is in the other pile', () => {
    expect(placed({ top: [1, 2], bottom: [3] }, 1, 'bottom', 2)).toEqual({ top: [2], bottom: [3, 1] })
  })
})

describe('scryPreview', () => {
  it('names both piles, top first, or none', () => {
    expect(scryPreview(['A', 'B'], ['C'])).toBe('Top of your library: A, then B. Bottom, top to bottom: C.')
    expect(scryPreview([], ['A', 'B'])).toBe('Top of your library: none. Bottom, top to bottom: A, then B.')
  })
})
