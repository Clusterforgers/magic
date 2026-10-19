// Goal: the words and rules of the card prompts hold for every min, max and number of cards on offer: the bounds never ask for more than is shown, a press changes a choice by the rules of one or several, the sentences say the numbers and what is left to do, the filter matches names and types, and an order says where a card went.
import { describe, expect, it } from 'vitest'
import type { ObjectView } from '@/protocol/generated'
import {
  boundsOf,
  cardKeysWords,
  cardPlaceWords,
  cardsAsk,
  cardsProgress,
  cardsRule,
  cardsSpoken,
  cardsTitle,
  countFits,
  distinctCards,
  matchesFilter,
  orderSentence,
  orderTitle,
  pressedCard,
} from './cardsText'

const view = (
  name: string,
  types: ObjectView['types'],
  subtypes: ObjectView['subtypes'] = [],
): ObjectView => ({
  id: 1,
  owner: 0,
  controller: 0,
  zone: 'Battlefield',
  name,
  mana_cost: null,
  supertypes: [],
  types,
  subtypes,
  colors: [],
  power: null,
  toughness: null,
  abilities: [],
  permanent: null,
})

describe('the bounds', () => {
  it.each([
    [1, 1, 3, 1, 1],
    [0, 2, 5, 0, 2],
    [2, 9, 4, 2, 4],
    [5, 5, 3, 3, 3],
    [1, 1, 0, 0, 0],
    [0, 0, 4, 0, 0],
  ])('min %i and max %i of %i cards ask for %i to %i', (min, max, total, lo, hi) => {
    expect(boundsOf(min, max, total)).toEqual({ lo, hi })
  })

  it.each([
    [0, 1, 1, false],
    [1, 1, 1, true],
    [2, 1, 1, false],
    [0, 0, 2, true],
    [2, 0, 2, true],
    [3, 0, 2, false],
  ])('%i cards against %i to %i is %s', (count, lo, hi, fits) => {
    expect(countFits(count, { lo, hi })).toBe(fits)
  })

  it('counts a card offered twice once', () => {
    expect(distinctCards([4, 7, 4, 9, 7])).toEqual([4, 7, 9])
  })
})

describe('a press', () => {
  it.each([
    ['chooses a card', [], 5, 2, [5]],
    ['chooses another after it', [3], 5, 2, [3, 5]],
    ['takes a chosen card back', [3, 5], 3, 2, [5]],
    ['takes the only card back', [5], 5, 1, []],
    ['changes a choice of one', [3], 5, 1, [5]],
    ['changes nothing when the choice is full', [3, 4], 5, 2, [3, 4]],
    ['chooses nothing when nothing may be chosen', [], 5, 0, []],
  ])('%s', (_what, chosen, id, hi, expected) => {
    expect(pressedCard(chosen, id, hi)).toEqual(expected)
  })

  it('returns the very same list when nothing changes, so a caller can tell', () => {
    const chosen = [3, 4]
    expect(pressedCard(chosen, 5, 2)).toBe(chosen)
  })
})

describe('the sentences', () => {
  it.each([
    [1, 1, 3, 'Choose a card.', 'Choose 1 of 3'],
    [2, 2, 7, 'Choose two cards.', 'Choose 2 of 7'],
    [0, 1, 4, 'Choose up to one card.', 'Choose up to 1 of 4'],
    [0, 3, 6, 'Choose up to three cards.', 'Choose up to 3 of 6'],
    [0, 9, 4, 'Choose any number of the cards.', 'Choose any number of 4'],
    [1, 9, 4, 'Choose at least one card.', 'Choose at least 1 of 4'],
    [2, 4, 9, 'Choose two to four cards.', 'Choose 2 to 4 of 9'],
    [0, 12, 40, 'Choose up to 12 cards.', 'Choose up to 12 of 40'],
    [0, 0, 4, 'No card can be chosen.', 'No card can be chosen'],
    [1, 1, 0, 'There is nothing to choose.', 'No cards to choose from'],
  ])('min %i and max %i of %i cards read "%s" and the count "%s"', (min, max, total, title, rule) => {
    const bounds = boundsOf(min, max, total)
    expect(cardsTitle(bounds, total)).toBe(title)
    expect(cardsRule(bounds, total)).toBe(rule)
  })

  it.each([
    [
      40,
      'Choose up to 12 of 40',
      'Choose up to 12 of 40. Press 1 to 9, or 0, to choose one of the first ten or take it back.',
    ],
    [10, null, 'Press 1 to 9, or 0, to choose one of the first ten or take it back.'],
    [3, null, 'Press 1 to 3 to choose a card or take it back.'],
    [1, null, 'Press 1 to choose it or take it back.'],
    [0, null, ''],
  ])('tells a screen reader the numbers and keys for %i cards in the list', (count, rule, spoken) => {
    expect(cardsSpoken(rule, cardKeysWords(count))).toBe(spoken)
  })

  it('opens with who asks and the card words', () => {
    expect(cardsAsk('Archon of Cruelty', 'Discard a card.')).toBe('Archon of Cruelty: Discard a card.')
  })

  it.each([
    [1, 2, 2, 'Brainstorm: 1 of 2 chosen. Choose 1 more.'],
    [2, 2, 2, 'Brainstorm: 2 cards chosen. Press Confirm, or take one back.'],
    [1, 0, 3, 'Brainstorm: 1 card chosen. Press Confirm, or choose up to 2 more.'],
    [3, 0, 3, 'Brainstorm: 3 cards chosen. Press Confirm, or take one back.'],
    [1, 1, 1, 'Brainstorm: 1 card chosen. Press Confirm, or choose another card instead.'],
  ])('with %i chosen of %i to %i says "%s"', (chosen, lo, hi, text) => {
    expect(cardsProgress('Brainstorm', chosen, { lo, hi })).toBe(text)
  })

  it('says nothing can be chosen when no card may be', () => {
    expect(cardsProgress('Brainstorm', 0, { lo: 0, hi: 0 })).toBe(
      'Brainstorm: nothing can be chosen. Press Confirm to go on.',
    )
  })
})

describe('the filter', () => {
  const bears = view('Grizzly Bears', ['Creature'], ['Bear'])
  const island = view('Island', ['Land'], ['Island'])

  it.each([
    ['', true, true],
    ['  ', true, true],
    ['bear', true, false],
    ['GRIZZLY', true, false],
    ['creature', true, false],
    ['land', false, true],
    ['grizzly bear', true, false],
    ['bear land', false, false],
    ['zzz', false, false],
  ])('"%s" matches the bears %s and the island %s', (filter, bearsMatch, islandMatch) => {
    expect(matchesFilter(bears, filter)).toBe(bearsMatch)
    expect(matchesFilter(island, filter)).toBe(islandMatch)
  })

  it('passes a card the view does not hold only while the filter is empty', () => {
    expect(matchesFilter(undefined, '')).toBe(true)
    expect(matchesFilter(undefined, 'bear')).toBe(false)
  })
})

describe('the order', () => {
  it.each([
    [1, 'Put the card in order.'],
    [3, 'Put the 3 cards in order.'],
  ])('of %i cards reads "%s"', (count, title) => {
    expect(orderTitle(count)).toBe(title)
  })

  it('names the asker and the last move', () => {
    expect(orderSentence('Opt', 'Put them back.', null)).toBe('Opt: Put them back.')
    expect(orderSentence('Opt', 'Put them back.', 'Island is now 2nd of 3.')).toBe(
      'Opt: Put them back. Island is now 2nd of 3.',
    )
  })

  it.each([
    [0, 3, 'now 1st of 3, first'],
    [1, 3, 'now 2nd of 3'],
    [2, 3, 'now 3rd of 3, last'],
    [0, 1, 'now 1st of 1, first'],
  ])('place %i of %i reads "%s"', (place, count, words) => {
    expect(cardPlaceWords(place, count)).toBe(words)
  })
})
