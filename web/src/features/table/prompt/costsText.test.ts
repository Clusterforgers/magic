/** Goal: the optional costs of a spell are cut into words and mana symbols as the card prints them, their extra cost adds the mana of the chosen ones and lists the parts that are not mana, a second gift is refused with a reason, and the answer lists the chosen costs in ascending order. */
import { describe, expect, it } from 'vitest'
import type { OptionalCost } from '@/protocol/generated'
import { cost } from '@/test-utils/mana'
import {
  costKeysWords,
  costsAnswer,
  costsSentence,
  extraCost,
  extraCostWords,
  labelPieces,
  parseSymbol,
  refusal,
  SEAT_BASE,
} from './costsText'

describe('labelPieces', () => {
  // A symbol is shown in angle brackets, words as they are.
  it.each<[string, string]>([
    ['Kicker {2}{R}', 'Kicker <2><R>'],
    ['Buyback {3}', 'Buyback <3>'],
    ['Entwine {C}', 'Entwine <C>'],
    ['Kicker {W/U}', 'Kicker {W/U}'],
    ['Gift to player 2', 'Gift to player 2'],
    ['Kicker {1}{G}, Discard a card', 'Kicker <1><G>, Discard a card'],
  ])('cuts "%s" into "%s"', (label, shown) => {
    const text = labelPieces(label)
      .map((piece) => ('symbol' in piece ? `<${piece.inner}>` : piece.text))
      .join('')
    expect(text).toBe(shown)
  })

  it.each([
    ['2', { Generic: 2 }],
    ['G', { Colored: 'Green' }],
    ['C', 'Colorless'],
    ['W/U', null],
    ['X', null],
  ])('reads the symbol {%s}', (inner, symbol) => {
    expect(parseSymbol(inner)).toEqual(symbol)
  })
})

describe('extraCost', () => {
  it.each<[string, string[], string, string[]]>([
    ['nothing chosen', [], '', []],
    ['one kicker', ['Kicker {2}{R}'], '{2}{R}', []],
    ['two kickers add up', ['Kicker {1}{G}', 'Buyback {3}'], '{4}{G}', []],
    ['colours in card order', ['Kicker {G}{W}', 'Kicker {R}{W}'], '{W}{W}{R}{G}', []],
    ['a gift is not mana', ['Gift to player 2'], '', ['Gift to player 2']],
    ['a part after the mana', ['Kicker {1}{G}, Discard a card'], '{1}{G}', ['Discard a card']],
    ['colourless', ['Kicker {C}{C}'], '{C}{C}', []],
  ])('%s', (_what, labels, mana, others) => {
    const extra = extraCost(labels)
    expect(extra.mana).toEqual(mana === '' ? [] : cost(mana))
    expect(extra.others).toEqual(others)
  })

  it.each<[string[], string]>([
    [[], 'nothing'],
    [['Kicker {2}{R}'], '{2}{R}'],
    [['Kicker {2}{R}', 'Gift to player 2'], '{2}{R} and Gift to player 2'],
    [['Gift to player 2'], 'Gift to player 2'],
  ])('says %j as "%s"', (labels, words) => {
    expect(extraCostWords(extraCost(labels))).toBe(words)
  })
})

describe('refusal', () => {
  const kicker: OptionalCost = { index: 0, label: 'Kicker {2}{R}' }
  const giftA: OptionalCost = { index: SEAT_BASE + 1, label: 'Gift to player 2' }
  const giftB: OptionalCost = { index: SEAT_BASE + 2, label: 'Gift to player 3' }
  const all = [kicker, giftA, giftB]

  it.each<[string, OptionalCost, number[], string | undefined]>([
    ['a kicker with nothing chosen', kicker, [], undefined],
    ['a kicker beside a gift', kicker, [giftA.index], undefined],
    ['a first gift', giftA, [], undefined],
    ['a gift beside a kicker', giftA, [0], undefined],
    ['the same gift again, to take it back', giftA, [giftA.index], undefined],
    [
      'a second gift',
      giftB,
      [giftA.index],
      'Only one gift can be promised: take back "Gift to player 2" to choose "Gift to player 3".',
    ],
  ])('%s', (_what, toggled, chosen, reason) => {
    expect(refusal(toggled, chosen, all)).toBe(reason)
  })
})

describe('the words and the answer', () => {
  it.each<[number, string]>([
    [0, 'Burst Lightning: choose the optional costs to pay, or none.'],
    [1, 'Burst Lightning: 1 optional cost chosen. Confirm, or change the choice.'],
    [3, 'Burst Lightning: 3 optional costs chosen. Confirm, or change the choice.'],
  ])('says %i chosen', (chosen, sentence) => {
    expect(costsSentence('Burst Lightning', chosen)).toBe(sentence)
  })

  it.each<[number, string]>([
    [1, 'Press 1 to take the cost or leave it.'],
    [3, 'Press 1 to 3 to take a cost or leave it.'],
    [12, 'Press 1 to 9, or 0, to take one of the first ten costs or leave it.'],
  ])('names the keys for %i costs', (count, words) => {
    expect(costKeysWords(count)).toBe(words)
  })

  it('lists the chosen costs in ascending order', () => {
    expect(costsAnswer([SEAT_BASE + 1, 1, 0])).toEqual([0, 1, SEAT_BASE + 1])
    expect(costsAnswer([])).toEqual([])
  })
})
