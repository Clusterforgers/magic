/** Goal: the PayMana prompt tells the player in plain words what a cost asks for (hybrid and Phyrexian symbols included), what is still owed, what the last click did and what to do next (choose a source, pay life, confirm, remove a choice or cancel). */
import { describe, expect, it } from 'vitest'
import { cost } from '@/test-utils/mana'
import { costWords, manaName, payText } from './payText'
import type { PayTextInput } from './payText'

const NOTHING = cost('')

describe('costWords', () => {
  it.each([
    ['a free spell', '', 'nothing'],
    ['generic only', '{3}', '3 generic'],
    ['one colour', '{G}', '1 green'],
    ['generic and a colour', '{2}{G}', '2 generic and 1 green'],
    ['three kinds', '{1}{W}{G}', '1 generic, 1 white and 1 green'],
    ['colourless', '{1}{C}{C}', '1 generic and 2 colorless'],
    ['five colours', '{W}{U}{B}{R}{G}', '1 white, 1 blue, 1 black, 1 red and 1 green'],
    ['everything', '{2}{C}{U}{U}', '2 generic, 1 colorless and 2 blue'],
    ['a two-colour hybrid symbol', '{W/U}', '1 white or blue'],
    ['equal hybrid symbols counted together', '{R/W}{R/W}{R/W}', '3 red or white'],
    ['a monocolored hybrid symbol', '{2/W}', '1 white or 2 generic'],
    ['a Phyrexian symbol', '{W/P}', '1 white or 2 life'],
    ['two equal Phyrexian symbols', '{1}{B/P}{B/P}', '1 generic and 2 black or 2 life'],
    ['a hybrid Phyrexian symbol', '{G/U/P}', '1 green or blue or 2 life'],
    ['a colorless hybrid symbol', '{C/W}', '1 colorless or white'],
    ['the colorless Phyrexian symbol', '{C/P}', '1 colorless or 2 life'],
    ['a mixed cost', '{2}{W/P}{G/U/P}', '2 generic, 1 white or 2 life and 1 green or blue or 2 life'],
  ])('%s', (_what, text, words) => {
    expect(costWords(cost(text))).toBe(words)
  })

  it('names the six kinds of mana', () => {
    expect(
      ['White', 'Blue', 'Black', 'Red', 'Green', 'Colorless'].map((m) => manaName(m as 'White')),
    ).toEqual(['white', 'blue', 'black', 'red', 'green', 'colorless'])
  })
})

const GROWTH = cost('{G}')
const EVANGEL = cost('{2}{W}{G}')
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
      input({ chosen: 1, owed: cost('{2}{G}'), note: 'Added white from Plains.' }),
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

describe('payText with a cost that life can pay', () => {
  const GITAXIAN = cost('{1}{U/P}')
  const life = (paid: number, may = true) => ({ paid, after: 20 - paid, may })
  const probe = (over: Partial<PayTextInput>): PayTextInput =>
    input({ spell: 'Gitaxian Probe', cost: GITAXIAN, owed: GITAXIAN, life: life(0), ...over })

  it.each([
    [
      'offers life with the lands before anything is chosen',
      probe({}),
      'Pay 1 generic and 1 blue or 2 life for Gitaxian Probe. Choose a land or other mana source to use its mana, pay 2 life for a Phyrexian symbol, or pay automatically.',
    ],
    [
      'offers life too when the game offers no automatic payment but life can pay',
      probe({ auto: false }),
      'Pay 1 generic and 1 blue or 2 life for Gitaxian Probe. Choose a land or other mana source to use its mana, pay 2 life for a Phyrexian symbol.',
    ],
    [
      'says it cannot be paid when neither mana nor life can',
      probe({ auto: false, canAdd: false, life: life(0, false) }),
      'You cannot pay 1 generic and 1 blue or 2 life for Gitaxian Probe with the mana you have. Cancel to go back.',
    ],
    [
      'says what is left after life is chosen',
      probe({
        chosen: 1,
        owed: cost('{1}'),
        life: life(2, false),
        note: 'Paying 2 life instead of blue mana.',
      }),
      'Paying 2 life instead of blue mana. Still to pay: 1 generic. Choose more, or pay the rest for me.',
    ],
    [
      'offers life again while a symbol is still unpaid',
      probe({ chosen: 1, owed: cost('{U/P}'), note: 'Added green from Forest.' }),
      'Added green from Forest. Still to pay: 1 blue or 2 life. Choose more, pay 2 life for a Phyrexian symbol, or pay the rest for me.',
    ],
    [
      'counts the life paid when the cost is covered',
      probe({
        chosen: 2,
        owed: NOTHING,
        covered: true,
        life: { paid: 2, after: 18, may: false },
        note: 'Added green from Forest.',
      }),
      'Added green from Forest. The cost is covered, paying 2 life (18 left). Confirm to cast Gitaxian Probe.',
    ],
    [
      'says nothing about life once it is covered by mana alone',
      probe({ chosen: 2, owed: NOTHING, covered: true, life: life(0) }),
      'The cost is covered. Confirm to cast Gitaxian Probe.',
    ],
  ])('%s', (_what, given, words) => {
    expect(payText(given)).toBe(words)
  })
})
