/** Goal: a choice of cards is a scry or surveil split only for library cards asked in the engine's words with 0 to all of them allowed, the split keeps the order looked at for 0, 1, 2, 3 and 5 cards, and the words of the split and of the order (piles, preview, keys, which end of the library) are exact. */
import { describe, expect, it } from 'vitest'
import type { Zone } from '@/protocol/generated'
import {
  awayOf,
  awayPile,
  lookKeys,
  lookPreview,
  lookProgress,
  lookTitle,
  orderPreview,
  pileHeading,
  pileOf,
  splitLooked,
} from './lookText'

const SCRY = 'Put any number of the cards on the bottom of your library.'
const SURVEIL = 'Put any number of the cards into your graveyard.'
const library: Zone = { Library: 0 }
const hand: Zone = { Hand: 0 }
const zones = (count: number, zone: Zone = library): Zone[] => Array.from({ length: count }, () => zone)

describe('awayOf', () => {
  it.each([
    ['a scry of 4', { min: 0, max: 4, text: SCRY }, zones(4), 'bottom'],
    ['a scry of 1 (the mulligan scry)', { min: 0, max: 1, text: SCRY }, zones(1), 'bottom'],
    ['a surveil of 3', { min: 0, max: 3, text: SURVEIL }, zones(3), 'graveyard'],
    ['a scry whose max is above the cards on offer', { min: 0, max: 9, text: SCRY }, zones(2), 'bottom'],
    [
      'a search of the library',
      { min: 0, max: 2, text: 'Search your library for up to two cards.' },
      zones(5),
      null,
    ],
    ['a scry that must choose one', { min: 1, max: 4, text: SCRY }, zones(4), null],
    ['a scry that may not take all', { min: 0, max: 3, text: SCRY }, zones(4), null],
    ['cards of a hand in the same words', { min: 0, max: 2, text: SCRY }, zones(2, hand), null],
    ['a card the view does not hold', { min: 0, max: 2, text: SCRY }, [library, undefined], null],
    ['no words', { min: 0, max: 2 }, zones(2), null],
    ['nothing on offer', { min: 0, max: 0, text: SCRY }, [], null],
  ])('reads %s', (_what, question, where, expected) => {
    expect(awayOf(question, where.length, where)).toBe(expected)
  })

  it('needs one zone for each card on offer', () => {
    expect(awayOf({ min: 0, max: 3, text: SCRY }, 3, zones(2))).toBeNull()
  })
})

describe('splitLooked', () => {
  const looked = [10, 11, 12, 13, 14]
  it.each([
    [0, [], []],
    [1, [], [10]],
    [2, [11], [10]],
    [3, [10, 11, 12], []],
    [5, [14, 10, 12], [11, 13]],
  ])('of %i cards sends %j away in the order looked at', (count, away, top) => {
    const cards = looked.slice(0, count)
    const split = splitLooked(cards, new Set(away))
    expect(split.away).toEqual(cards.filter((id) => away.includes(id)))
    expect(split.top).toEqual(top)
    expect([...split.top, ...split.away].toSorted((a, b) => a - b)).toEqual(cards)
  })
})

describe('the words', () => {
  it.each([
    ['bottom', 'On the bottom'],
    ['graveyard', 'In the graveyard'],
  ] as const)('name the away pile of a %s split "%s"', (kind, pile) => {
    expect(awayPile(kind)).toBe(pile)
  })

  it.each([
    ['bottom', 1, 'Put the card on the bottom of your library, or keep it on top.'],
    ['bottom', 3, 'Choose which of the cards go on the bottom of your library.'],
    ['graveyard', 2, 'Choose which of the cards go into your graveyard.'],
  ] as const)('title a %s split of %i cards', (kind, count, title) => {
    expect(lookTitle(kind, count)).toBe(title)
  })

  it.each([
    [3, 1, 'bottom', 'Opt: 3 cards stay on top, 1 goes to the bottom. Press Confirm when it is right.'],
    [1, 2, 'bottom', 'Opt: 1 card stays on top, 2 go to the bottom. Press Confirm when it is right.'],
    [0, 4, 'graveyard', 'Opt: None stay on top, 4 go to the graveyard. Press Confirm when it is right.'],
    [2, 0, 'bottom', 'Opt: 2 cards stay on top, none go to the bottom. Press Confirm when it is right.'],
  ] as const)('tell progress of %i on top and %i away (%s)', (top, away, kind, said) => {
    expect(lookProgress('Opt', top, away, kind)).toBe(said)
  })

  it.each([
    [1, 'Press 1 to move the card, then Confirm.'],
    [4, 'Press 1 to 4 to move that card between the top and the other pile, the top card being 1.'],
    [12, 'Press 1 to 9, or 0, to move that card between the top and the other pile, the top card being 1.'],
  ])('tell the keys for %i cards', (count, keys) => {
    expect(lookKeys(count)).toBe(keys)
  })

  it('previews both piles', () => {
    expect(lookPreview(['A', 'B'], ['C'], 'bottom')).toBe('On top: A, B. On the bottom: C.')
    expect(lookPreview([], ['C', 'D'], 'graveyard')).toBe('On top: none. In the graveyard: C, D.')
    expect(lookPreview(['A'], [], 'bottom')).toBe('On top: A. On the bottom: none.')
  })
})

describe('the order of the cards put back', () => {
  it.each([
    ['Put the cards on top of your library in any order.', 'top'],
    ['Put the cards on the bottom of your library in any order.', 'bottom'],
    ['Order.', 'top'],
    ['', 'top'],
  ] as const)('reads "%s" as going to the %s', (words, pile) => {
    expect(pileOf(words)).toBe(pile)
  })

  it('heads and previews each end', () => {
    expect(pileHeading('top')).toBe('Going on top of your library')
    expect(pileHeading('bottom')).toBe('Going on the bottom of your library')
    expect(orderPreview(['A', 'B', 'C'], 'top')).toBe('Top of your library: A, then B, then C.')
    expect(orderPreview(['A'], 'bottom')).toBe('Bottom of your library, top to bottom: A.')
  })
})
