// Goal: mana whose uses are riders alone ("when that mana is spent to cast ...") limits nothing, so it counts as carrying a rider and not as restricted, while a unit with any other use besides is restricted; the riders of a kind are listed once each, a spell filter is worded as a card says it, and a rider is worded by its source's own sentence when it has one.
import { describe, expect, it } from 'vitest'
import type { ManaPool, Restricted, SpendFilter, SpendOn, WhenSpent } from '@/protocol/generated'
import { card } from '@/test-utils/cards'
import { limits, manaRiders, riderKey, riderWords, spendFilterWords } from './manaRider'
import { manaRestricted } from './ManaPool'

const COMMANDER_CREATURE: SpendFilter = { types: ['Creature'], facts: ['SharesCommanderType'] }
const rider = (spell: SpendFilter = COMMANDER_CREATURE, source = 7): WhenSpent => ({
  spell,
  effects: [{ Scry: { count: 1 } }],
  source,
})
const WHEN = (over?: WhenSpent): SpendOn => ({ When: over ?? rider() })
const unit = (mana: Restricted['mana'], ...only: SpendOn[]): Restricted => ({ mana, only })
const pool = (...restricted: Restricted[]): ManaPool => ({ counts: [0, 0, 0, 0, 0, 0], restricted })

describe('which mana is restricted', () => {
  it.each<[string, Restricted, boolean]>([
    ['a rider alone', unit('White', WHEN()), false],
    ['two riders', unit('White', WHEN(), WHEN(rider({ types: ['Land'] }))), false],
    ['a use alone', unit('White', 'CostWithX'), true],
    ['a spell use', unit('White', { Spell: { types: ['Creature'] } }), true],
    ['a rider beside a use', unit('White', WHEN(), 'CostWithX'), true],
    ['no use at all', unit('White'), false],
  ])('%s: %s', (_case, restricted, expected) => {
    expect(limits(restricted)).toBe(expected)
  })

  it.each<[string, Restricted[], Array<[string, number]>]>([
    ['rider mana is not restricted', [unit('White', WHEN())], []],
    ['a use is', [unit('Colorless', 'CostWithX')], [['Colorless', 1]]],
    [
      'only the units with a use, by kind in the order of the wire',
      [
        unit('Red', 'CostWithX'),
        unit('White', WHEN()),
        unit('Red', WHEN(), 'CostWithX'),
        unit('Blue', WHEN()),
      ],
      [['Red', 2]],
    ],
  ])('the pool counts %s', (_case, units, expected) => {
    expect(manaRestricted(pool(...units)).map(({ type, count }) => [type, count])).toEqual(expected)
  })
})

describe('manaRiders', () => {
  it('lists the mana that carries riders alone, by kind in the order of the wire', () => {
    const found = manaRiders(
      pool(unit('Green', WHEN()), unit('White', WHEN()), unit('Green', WHEN()), unit('Red', 'CostWithX')),
    )
    expect(found.map(({ type, count }) => [type, count])).toEqual([
      ['White', 1],
      ['Green', 2],
    ])
  })

  it('lists a rider once however many units carry it, and each different rider', () => {
    const other = rider({ types: ['Artifact'] }, 8)
    const [white] = manaRiders(
      pool(unit('White', WHEN()), unit('White', WHEN(), WHEN(other)), unit('White', WHEN())),
    )
    expect(white?.riders.map(riderKey)).toEqual([riderKey(rider()), riderKey(other)])
  })

  it('is none for a pool with nothing restricted', () => {
    expect(manaRiders({ counts: [1, 0, 0, 0, 0, 0] })).toEqual([])
  })
})

describe('spendFilterWords', () => {
  it.each<[string, SpendFilter, string]>([
    [
      'the commander rider of Path of Ancestry',
      COMMANDER_CREATURE,
      'a creature spell that shares a creature type with your commander',
    ],
    ['any spell', {}, 'a spell'],
    ['a colour', { colors: ['Red'] }, 'a red spell'],
    ['two colours', { colors: ['Red', 'Green'] }, 'a red or green spell'],
    ['a multicoloured spell', { multicolored: true }, 'a multicolored spell'],
    ['a type to avoid', { without: ['Creature'] }, 'a noncreature spell'],
    ['a creature type', { types: ['Creature'], subtypes: ['Elf'] }, 'an elf creature spell'],
    ['a supertype', { supertypes: ['Legendary'], types: ['Creature'] }, 'a legendary creature spell'],
    ['the chosen type', { chosen_type: true, types: ['Creature'] }, 'a chosen-type creature spell'],
    ['a mana value', { min_value: 5 }, 'a spell with mana value 5 or greater'],
    ['{X} in the cost', { facts: ['XInCost'] }, 'a spell with {X} in its cost'],
    ['a graveyard', { facts: ['FromGraveyard'] }, 'a spell cast from a graveyard'],
  ])('words %s', (_case, filter, expected) => {
    expect(spendFilterWords(filter)).toBe(expected)
  })
})

describe('riderWords', () => {
  const source = card({
    id: 7,
    name: 'Path of Ancestry',
    activated: [
      { cost: '{T}', effect: 'Add {W}.', mana: true },
      {
        cost: '{T}',
        effect:
          'Add one mana of any color in your commander’s color identity. When that mana is spent to cast a creature spell that shares a creature type with your commander, scry 1.',
        mana: true,
      },
    ],
  })

  it('uses the source’s own sentence when its mana ability words the rider', () => {
    expect(riderWords(rider(), source)).toBe(
      'When that mana is spent to cast a creature spell that shares a creature type with your commander, scry 1.',
    )
  })

  it.each<[string, Parameters<typeof card>[0] | undefined, string]>([
    [
      'the source is not in the store',
      undefined,
      'When this mana is spent to cast a creature spell that shares a creature type with your commander, an ability of the permanent that made it triggers.',
    ],
    [
      'the source does not word it',
      { id: 7, name: 'Path of Ancestry', activated: [{ cost: '{T}', effect: 'Add {W}.', mana: true }] },
      'When this mana is spent to cast a creature spell that shares a creature type with your commander, an ability of Path of Ancestry triggers.',
    ],
  ])('names the kind of spell and the source when %s', (_case, over, expected) => {
    expect(riderWords(rider(), over === undefined ? undefined : card(over))).toBe(expected)
  })
})
