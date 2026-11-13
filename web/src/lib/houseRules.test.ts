// Goal: the house rules table reads field text into the request's rules exactly within the server's ranges (handicaps per seat of the table, in seat order), names only what differs from the format's own values, and shows Commander's rules only in Commander.
import { describe, expect, it } from 'vitest'
import type { HouseRules } from '@/protocol/generated'
import {
  describeRules,
  gameFormat,
  handicapId,
  requestRules,
  RULES,
  ruleGroups,
  rulesFromTexts,
} from './houseRules'
import type { GameFormat } from './houseRules'

describe('reading field text', () => {
  const cases: [string, Record<string, string>, GameFormat, HouseRules][] = [
    ['nothing typed', {}, 'commander', {}],
    ['empty and blank texts', { starting_life: '', free_mulligans: '  ' }, 'commander', {}],
    ['a count', { starting_life: '30' }, 'commander', { starting_life: 30 }],
    ['a count with spaces', { opening_hand_size: ' 6 ' }, 'normal', { opening_hand_size: 6 }],
    ['a zero that means none', { mulligan_cap: '0' }, 'normal', { mulligan_cap: 0 }],
    ['a hand limit', { max_hand_size: '9' }, 'normal', { max_hand_size: { cards: 9 } }],
    ['no hand limit in any case', { max_hand_size: 'Unlimited' }, 'normal', { max_hand_size: 'unlimited' }],
    ['commander damage', { commander_damage: '15' }, 'commander', { commander_damage: { lethal: 15 } }],
    ['commander damage off', { commander_damage: 'off' }, 'commander', { commander_damage: 'off' }],
    ['a choice', { mulligan_style: 'vancouver' }, 'normal', { mulligan_style: 'vancouver' }],
    ['a switch', { mana_burn: 'on', open_hands: 'off' }, 'normal', { mana_burn: true, open_hands: false }],
    ['take-backs turned off', { take_backs: 'off' }, 'commander', { take_backs: false }],
    ['take-backs left on, which is not sent', { take_backs: '' }, 'normal', {}],
    ['take-backs on, which is not sent either', { take_backs: 'on' }, 'normal', {}],
    [
      'deck rules gather in one object',
      { 'deck.singleton': 'off', 'deck.max_copies': '3', 'deck.deck_size': '60' },
      'commander',
      { deck: { singleton: false, max_copies: 3, deck_size: 60 } },
    ],
    ['a Commander rule typed, then the format changed to Normal', { commander_tax: '0' }, 'normal', {}],
  ]
  it.each(cases)('%s', (_name, texts, format, rules) => {
    const read = rulesFromTexts(texts, format)
    expect(read.problems).toEqual({})
    expect(read.rules).toEqual(rules)
  })

  const refused: [string, string, string, GameFormat][] = [
    ['life of zero', 'starting_life', '0', 'normal'],
    ['life over the range', 'starting_life', '1001', 'normal'],
    ['a hand of 21', 'opening_hand_size', '21', 'normal'],
    ['words for a count', 'free_mulligans', 'two', 'normal'],
    ['a negative count', 'free_mulligans', '-1', 'normal'],
    ['a fraction', 'draw_step_cards', '1.5', 'normal'],
    ['a hand limit of 51', 'max_hand_size', '51', 'normal'],
    ['a deck of 39', 'deck.deck_size', '39', 'commander'],
    ['3 commanders', 'commanders_per_seat', '3', 'commander'],
    ['an unknown style', 'mulligan_style', 'paris-ish', 'normal'],
    ['a switch worded freely', 'mana_burn', 'yes', 'normal'],
    ['take-backs worded freely', 'take_backs', 'no', 'normal'],
    ['a tax of 11', 'commander_tax', '11', 'commander'],
  ]
  it.each(refused)('names the field of %s', (_name, id, text, format) => {
    const read = rulesFromTexts({ [id]: text }, format)
    expect(Object.keys(read.problems)).toEqual([id])
    expect(read.rules).toEqual({})
  })

  it('asks for singleton off before more copies of a card', () => {
    expect(Object.keys(rulesFromTexts({ 'deck.max_copies': '2' }, 'commander').problems)).toEqual([
      'deck.max_copies',
    ])
    expect(
      rulesFromTexts({ 'deck.max_copies': '2', 'deck.singleton': 'on' }, 'commander').problems,
    ).toHaveProperty('deck.max_copies')
  })

  it('sends no rules when none is set', () => {
    expect(requestRules({}, 'commander')).toBeUndefined()
    expect(requestRules({ take_backs: '' }, 'normal')).toBeUndefined()
    expect(requestRules({ take_backs: 'off' }, 'normal')).toEqual({ take_backs: false })
    expect(requestRules({ starting_life: '' }, 'commander')).toBeUndefined()
    expect(requestRules({ starting_life: '30' }, 'commander')).toEqual({ starting_life: 30 })
  })
})

describe('naming the rules that differ', () => {
  const cases: [string, HouseRules | undefined, string, string[]][] = [
    ['no rules', undefined, 'commander', []],
    ['empty rules', {}, 'commander', []],
    ['life that is the format’s', { starting_life: 40 }, 'commander', []],
    ['life that is not', { starting_life: 30 }, 'commander', ['Starting life 30']],
    ['Normal’s life in Commander', { starting_life: 20 }, 'commander', ['Starting life 20']],
    ['40 life in Normal', { starting_life: 40 }, 'normal', ['Starting life 40']],
    ['a free mulligan in Commander', { free_mulligans: 1 }, 'commander', []],
    ['one in Normal', { free_mulligans: 1 }, 'normal', ['1 free mulligan']],
    ['no commander tax', { commander_tax: 0 }, 'commander', ['No commander tax']],
    ['a tax in Normal, which has none', { commander_tax: 0 }, 'normal', []],
    ['the format’s damage', { commander_damage: { lethal: 21 } }, 'commander', []],
    ['damage off', { commander_damage: 'off' }, 'commander', ['No commander damage loss']],
    ['a hand limit', { max_hand_size: 'unlimited' }, 'normal', ['No hand limit']],
    ['the format’s hand limit', { max_hand_size: { cards: 7 } }, 'normal', []],
    ['a switch that is the default', { mana_burn: false }, 'normal', []],
    ['a switch turned on', { open_hands: true }, 'normal', ['Open hands']],
    ['take-backs off', { take_backs: false }, 'commander', ['No take-backs']],
    ['take-backs on, as every game has them', { take_backs: true }, 'normal', []],
    ['a Vancouver mulligan', { mulligan_style: 'vancouver' }, 'commander', ['Vancouver mulligan']],
    ['the London mulligan', { mulligan_style: 'london' }, 'commander', []],
    [
      'a deck of 60 without singleton',
      { deck: { deck_size: 60, singleton: false } },
      'commander',
      ['Copies of a card unlimited', 'Decks of 60 cards'],
    ],
    ['deck rules in Normal, which ignores them', { deck: { deck_size: 60 } }, 'normal', []],
    [
      'handicaps',
      {
        handicaps: [
          { seat: 1, starting_life: 30 },
          { seat: 2, opening_hand_size: 5 },
        ],
      },
      'commander',
      ['Handicaps for 2 players'],
    ],
    [
      'several, in the form’s order',
      { winning_life: 60, starting_life: 30, open_hands: true },
      'normal',
      ['Starting life 30', 'Win at 60 life', 'Open hands'],
    ],
  ]
  it.each(cases)('%s', (_name, rules, format, words) => {
    expect(describeRules(rules, format)).toEqual(words)
  })

  it('names a format by its wire key in any case', () => {
    expect(gameFormat('Commander')).toBe('commander')
    expect(gameFormat('commander')).toBe('commander')
    expect(gameFormat('normal')).toBe('normal')
    expect(gameFormat('modern')).toBe('normal')
  })
})

describe('the table of rules', () => {
  it('has unique ids', () => {
    const ids = RULES.map((spec) => spec.id)
    expect(new Set(ids).size).toBe(ids.length)
  })

  it.each(RULES.map((spec) => [spec.id, spec] as const))(
    'names nothing when %s is set to the format’s own value',
    (_id, spec) => {
      for (const format of spec.formats) {
        const own = spec.fallback(format)
        if (own === '') continue
        const rules: HouseRules = {}
        expect(spec.apply(rules, own)).toBeNull()
        expect(spec.phrase(rules, format)).toBeNull()
      }
    },
  )

  it('shows the Commander group only in Commander', () => {
    expect(ruleGroups('commander').map((group) => group.title)).toContain('Commander')
    expect(ruleGroups('normal').map((group) => group.title)).not.toContain('Commander')
    expect(ruleGroups('normal').flatMap((group) => group.specs.map((spec) => spec.id))).not.toContain(
      'commander_tax',
    )
  })

  it('groups the rules in the order of the table', () => {
    const titles = ruleGroups('commander').map((group) => group.title)
    expect(new Set(titles).size).toBe(titles.length)
  })
})

describe('reading handicaps', () => {
  const life = (seat: number) => handicapId(seat, 'starting_life')
  const hand = (seat: number) => handicapId(seat, 'opening_hand_size')
  const cases: [string, Record<string, string>, number, HouseRules][] = [
    ['none typed', {}, 4, {}],
    ['blank texts', { [life(0)]: ' ', [hand(1)]: '' }, 4, {}],
    ['one seat’s life', { [life(1)]: '30' }, 4, { handicaps: [{ seat: 1, starting_life: 30 }] }],
    [
      'both values of a seat',
      { [life(0)]: '10', [hand(0)]: ' 5 ' },
      2,
      {
        handicaps: [{ seat: 0, starting_life: 10, opening_hand_size: 5 }],
      },
    ],
    [
      'seats in table order',
      { [hand(3)]: '8', [life(0)]: '25' },
      4,
      {
        handicaps: [
          { seat: 0, starting_life: 25 },
          { seat: 3, opening_hand_size: 8 },
        ],
      },
    ],
    [
      'a seat the table no longer has',
      { [life(3)]: '30', [life(1)]: '9' },
      2,
      {
        handicaps: [{ seat: 1, starting_life: 9 }],
      },
    ],
    [
      'with a table rule',
      { starting_life: '30', [life(1)]: '40' },
      2,
      {
        starting_life: 30,
        handicaps: [{ seat: 1, starting_life: 40 }],
      },
    ],
  ]
  it.each(cases)('%s', (_name, texts, seats, rules) => {
    const read = rulesFromTexts(texts, 'commander', seats)
    expect(read.problems).toEqual({})
    expect(read.rules).toEqual(rules)
    expect(requestRules(texts, 'normal', seats)).toEqual(Object.keys(rules).length === 0 ? undefined : rules)
  })

  it.each([
    [life(0), '0'],
    [life(0), '1001'],
    [life(1), '-5'],
    [hand(0), '0'],
    [hand(1), '21'],
    [hand(1), 'six'],
  ])('refuses %s = %j and sends nothing for it', (id, text) => {
    const read = rulesFromTexts({ [id]: text }, 'commander', 2)
    expect(Object.keys(read.problems)).toEqual([id])
    expect(read.rules.handicaps).toBeUndefined()
  })

  it.each([
    [life(0), '1'],
    [life(0), '1000'],
    [hand(1), '1'],
    [hand(1), '20'],
  ])('takes %s = %j at the edge of its range', (id, text) => {
    expect(rulesFromTexts({ [id]: text }, 'commander', 2).problems).toEqual({})
  })

  it('names the seats with a handicap last', () => {
    const rules = rulesFromTexts({ [life(0)]: '30', [hand(2)]: '6' }, 'commander', 4).rules
    expect(describeRules(rules, 'commander')).toEqual(['Handicaps for 2 players'])
  })
})
