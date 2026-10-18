// Goal: the facts a card face draws are derived correctly for every kind of permanent: its row, cost symbols in card order, type line, power and toughness, frame colour, state labels and the full spoken description.
import { describe, expect, it } from 'vitest'
import type { CardType, Color, CombatView, ManaCost, ManaType } from '@/protocol/generated'
import { card, forest, permanent } from '@/test-utils/cards'
import {
  abilityText,
  abilityWords,
  cardLabel,
  colorsInSet,
  combatRoleOf,
  costPips,
  costText,
  frameOf,
  manaLetter,
  powerToughness,
  rowOf,
  statusLabels,
  statusWords,
  typeLine,
} from './cardModel'

const cost = (generic: number, colored: ManaCost['colored'], colorless = 0): ManaCost => ({
  generic,
  colorless,
  colored,
})

describe('rowOf', () => {
  it.each<[CardType[], string]>([
    [['Creature'], 'creatures'],
    [['Land'], 'lands'],
    [['Artifact', 'Creature'], 'creatures'],
    [['Land', 'Creature'], 'creatures'],
    [['Artifact', 'Land'], 'lands'],
    [['Artifact'], 'other'],
    [['Enchantment'], 'other'],
    [['Planeswalker'], 'other'],
    [['Battle'], 'other'],
    [['Instant'], 'other'],
  ])('puts %j in the %s row', (types, row) => {
    expect(rowOf(card({ types }))).toBe(row)
  })
})

describe('costPips', () => {
  it.each<[string, ManaCost, string[]]>([
    ['a generic and two colours, in card order', cost(2, [0, 1, 0, 1, 0]), ['2', 'U', 'R']],
    ['all five colours in WUBRG order', cost(0, [1, 1, 1, 1, 1]), ['W', 'U', 'B', 'R', 'G']],
    ['colourless symbols after the generic amount', cost(1, [0, 0, 0, 0, 1], 2), ['1', 'C', 'C', 'G']],
    ['a doubled colour', cost(0, [0, 0, 2, 0, 0]), ['B', 'B']],
    ['a zero cost as {0}', cost(0, [0, 0, 0, 0, 0]), ['0']],
  ])('lists %s', (_what, mana, texts) => {
    expect(costPips(mana).map((pip) => pip.text)).toEqual(texts)
  })

  it('gives each symbol a unique key, a tone and a spoken name', () => {
    const pips = costPips(cost(3, [0, 0, 2, 1, 0]))
    expect(new Set(pips.map((pip) => pip.key)).size).toBe(pips.length)
    expect(pips.map((pip) => pip.tone)).toEqual(['generic', 'black', 'black', 'red'])
    expect(pips.map((pip) => pip.name)).toEqual(['3 generic', 'black', 'black', 'red'])
  })

  it('writes a cost the way a card does', () => {
    expect(costText(cost(2, [0, 1, 0, 1, 0]))).toBe('{2}{U}{R}')
    expect(costText(cost(0, [0, 0, 0, 0, 0]))).toBe('{0}')
  })
})

describe('typeLine', () => {
  it.each([
    [card({ supertypes: ['Legendary'], subtypes: ['Elf', 'Warrior'] }), 'Legendary Creature — Elf Warrior'],
    [card({ subtypes: [] }), 'Creature'],
    [forest(), 'Basic Land — Forest'],
    [card({ types: ['Artifact', 'Creature'], subtypes: ['Golem'] }), 'Artifact Creature — Golem'],
  ])('prints %#', (object, text) => {
    expect(typeLine(object)).toBe(text)
  })
})

describe('powerToughness', () => {
  it('prints both numbers, negative ones included', () => {
    expect(powerToughness(card({ power: 3, toughness: 4 }))).toBe('3/4')
    expect(powerToughness(card({ power: -1, toughness: 1 }))).toBe('-1/1')
  })

  it.each([[forest()], [card({ power: 2, toughness: null })], [card({ power: null, toughness: 2 })]])(
    'is null without both numbers (%#)',
    (object) => {
      expect(powerToughness(object)).toBeNull()
    },
  )
})

describe('frameOf', () => {
  it.each<[Color[], string]>([
    [[], 'colorless'],
    [['White'], 'white'],
    [['Blue'], 'blue'],
    [['Black'], 'black'],
    [['Red'], 'red'],
    [['Green'], 'green'],
    [['Red', 'Green'], 'gold'],
    [['White', 'Blue', 'Black', 'Red', 'Green'], 'gold'],
  ])('draws %j in a %s frame', (colors, frame) => {
    expect(frameOf(card({ colors }))).toBe(frame)
  })
})

describe('colorsInSet', () => {
  it.each<[number, Color[]]>([
    [0, []],
    [0b00001, ['White']],
    [0b01010, ['Blue', 'Red']],
    [0b10000, ['Green']],
    [0b11111, ['White', 'Blue', 'Black', 'Red', 'Green']],
  ])('reads the mask %i', (mask, colors) => {
    expect(colorsInSet(mask)).toEqual(colors)
  })
})

describe('abilities', () => {
  it.each<[ManaType, string, string]>([
    ['Green', '{T}: Add {G}.', 'Tap: add green mana'],
    ['Colorless', '{T}: Add {C}.', 'Tap: add colorless mana'],
    ['White', '{T}: Add {W}.', 'Tap: add white mana'],
  ])('writes a %s mana ability', (produces, text, words) => {
    expect(abilityText({ Mana: { produces } })).toBe(text)
    expect(abilityWords({ Mana: { produces } })).toBe(words)
    expect(manaLetter(produces)).toBe(text.slice(-3, -2))
  })
})

describe('statusLabels and statusWords', () => {
  it('is empty for a plain untapped permanent and for a card that is not a permanent', () => {
    expect(statusLabels(permanent({}), null)).toEqual([])
    expect(statusLabels(card({ zone: { Hand: 0 }, permanent: null }), null)).toEqual([])
    expect(statusWords(permanent({}), null)).toEqual([])
  })

  it.each([
    [{ tapped: true }, null, ['Tapped'], ['tapped']],
    [{ summoning_sick: true }, null, ['Sick'], ['summoning sick']],
    [{ damage: 2 }, null, ['2 dmg'], ['2 damage']],
    [{}, 'attacking', ['Attacking'], ['attacking']],
    [{}, 'blocked', ['Blocked'], ['attacking, blocked']],
    [{}, 'blocking', ['Blocking'], ['blocking']],
    [
      { tapped: true, summoning_sick: true, damage: 1 },
      'blocking',
      ['Tapped', 'Sick', '1 dmg', 'Blocking'],
      ['tapped', 'summoning sick', '1 damage', 'blocking'],
    ],
  ] as const)('says %j in combat role %s', (state, role, labels, words) => {
    expect(statusLabels(permanent(state), role)).toEqual(labels)
    expect(statusWords(permanent(state), role)).toEqual(words)
  })
})

describe('cardLabel', () => {
  it('names the card, its cost, type, power and toughness and every state', () => {
    const label = cardLabel(permanent({ tapped: true, damage: 1 }, { name: 'Hill Giant' }), 'attacking')
    expect(label).toBe(
      'Hill Giant, cost 1 generic, green, Creature — Bear, power 2, toughness 2, tapped, 1 damage, attacking',
    )
  })

  it('names a land by its ability and leaves out a cost it does not have', () => {
    expect(cardLabel(forest(), null)).toBe('Forest, Basic Land — Forest, Tap: add green mana')
  })
})

describe('combatRoleOf', () => {
  const combat: CombatView = {
    attackers: [
      { object: 10, target: { Player: 0 }, blocked: true, blockers: [20, 21] },
      { object: 11, target: { Player: 0 }, blocked: false, blockers: [] },
    ],
    attackers_declared: true,
    blockers_declared: [0],
  }

  it.each([
    [10, 'blocked'],
    [11, 'attacking'],
    [20, 'blocking'],
    [21, 'blocking'],
    [99, null],
  ])('gives object %i the role %s', (id, role) => {
    expect(combatRoleOf(combat, id)).toBe(role)
  })

  it('has no roles when nothing attacks', () => {
    const quiet: CombatView = { attackers: [], attackers_declared: false, blockers_declared: [] }
    expect(combatRoleOf(quiet, 10)).toBeNull()
  })
})
