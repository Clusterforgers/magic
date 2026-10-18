/** Goal: the PayMana prompt tells the player in plain words what a cost asks for, what is still owed, what the last click did and what to do next (choose a source, confirm, remove a choice or cancel). */
import { describe, expect, it } from 'vitest'
import type { ManaCost } from '@/protocol/generated'
import { costWords, manaName, payText } from './payText'
import type { PayTextInput } from './payText'

const cost = (generic: number, colored: ManaCost['colored'] = [0, 0, 0, 0, 0], colorless = 0): ManaCost => ({
  generic,
  colorless,
  colored,
})
const NOTHING = cost(0)

describe('costWords', () => {
  it.each([
    ['a free spell', NOTHING, 'nothing'],
    ['generic only', cost(3), '3 generic'],
    ['one colour', cost(0, [0, 0, 0, 0, 1]), '1 green'],
    ['generic and a colour', cost(2, [0, 0, 0, 0, 1]), '2 generic and 1 green'],
    ['three kinds', cost(1, [1, 0, 0, 0, 1]), '1 generic, 1 white and 1 green'],
    ['colourless', cost(1, [0, 0, 0, 0, 0], 2), '1 generic and 2 colorless'],
    ['five colours', cost(0, [1, 1, 1, 1, 1]), '1 white, 1 blue, 1 black, 1 red and 1 green'],
    ['everything', cost(2, [0, 2, 0, 0, 0], 1), '2 generic, 1 colorless and 2 blue'],
  ])('%s', (_what, given, words) => {
    expect(costWords(given)).toBe(words)
  })

  it('names the six kinds of mana', () => {
    expect(
      ['White', 'Blue', 'Black', 'Red', 'Green', 'Colorless'].map((m) => manaName(m as 'White')),
    ).toEqual(['white', 'blue', 'black', 'red', 'green', 'colorless'])
  })
})

const GROWTH = cost(0, [0, 0, 0, 0, 1])
const EVANGEL = cost(2, [1, 0, 0, 0, 1])
const input = (over: Partial<PayTextInput>): PayTextInput => ({
  spell: 'Selesnya Evangel',
  cost: EVANGEL,
  owed: EVANGEL,
  chosen: 0,
  covered: false,
  canAdd: true,
  auto: true,
  cancel: true,
  note: '',
  ...over,
})

describe('payText', () => {
  it.each([
    [
      'before anything is chosen',
      input({}),
      'Pay 2 generic, 1 white and 1 green for Selesnya Evangel. Choose a land or other mana source to use its mana, or pay automatically.',
    ],
    [
      'before anything is chosen, with no automatic payment, which means the cost cannot be paid',
      input({ auto: false }),
      'You cannot pay 2 generic, 1 white and 1 green for Selesnya Evangel with the mana you have. Cancel to go back.',
    ],
    [
      'before anything is chosen, with nothing that can be added',
      input({ canAdd: false }),
      'You cannot pay 2 generic, 1 white and 1 green for Selesnya Evangel with the mana you have. Cancel to go back.',
    ],
    [
      'with something chosen',
      input({ chosen: 1, owed: cost(2, [0, 0, 0, 0, 1]), note: 'Added white from Plains.' }),
      'Added white from Plains. Still to pay: 2 generic and 1 green. Choose more, or pay the rest for me.',
    ],
    [
      'with something chosen and no automatic payment, which leaves out the offer to pay the rest',
      input({ chosen: 1, owed: GROWTH, auto: false, note: 'Added white from Plains.' }),
      'Added white from Plains. Still to pay: 1 green.',
    ],
    [
      'with something chosen and nothing left to add',
      input({ chosen: 1, owed: GROWTH, canAdd: false }),
      'Still to pay: 1 green. Nothing left can pay it; remove a choice or cancel.',
    ],
    [
      'with something chosen, nothing left to add and no way to cancel',
      input({ chosen: 1, owed: GROWTH, canAdd: false, cancel: false }),
      'Still to pay: 1 green. Nothing left can pay it; remove a choice.',
    ],
    [
      'before anything is chosen, unpayable, with no way to cancel',
      input({ auto: false, cancel: false }),
      'You cannot pay 2 generic, 1 white and 1 green for Selesnya Evangel with the mana you have.',
    ],
    [
      'when the cost is covered',
      input({ chosen: 4, owed: NOTHING, covered: true, note: 'Added green from Forest.' }),
      'Added green from Forest. The cost is covered. Confirm to cast Selesnya Evangel.',
    ],
    [
      'when a free spell needs nothing',
      input({ cost: NOTHING, owed: NOTHING, covered: true, spell: 'Ornithopter' }),
      'Ornithopter costs nothing. Confirm to cast it.',
    ],
    [
      'after a removal that leaves nothing',
      input({ note: 'Cleared the payment.' }),
      'Cleared the payment. Pay 2 generic, 1 white and 1 green for Selesnya Evangel. Choose a land or other mana source to use its mana, or pay automatically.',
    ],
  ])('%s', (_what, given, words) => {
    expect(payText(given)).toBe(words)
  })
})
