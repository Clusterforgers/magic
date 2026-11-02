/** Goal: restricted mana may pay for exactly what its uses list, read against what the payment is for as the engine reads it (kind, types, subtypes, colours, value, facts, X), a rider alone limits nothing, and a payment keeps each restricted mana of the pool and each source to one use and sends the restricted mana it spends. */
import { describe, expect, it } from 'vitest'
import type {
  ManaOption,
  ManaPool,
  Restricted,
  Spend as PaySpend,
  SpendFilter,
  SpendOn,
} from '@/protocol/generated'
import { cost, poolOf, source } from '@/test-utils/mana'
import { allowed, covers, toPayment, usableByHand } from './payment'
import type { Reach, Spend } from './payment'
import { spendAllows, usesWords } from './spendRules'

// Types are one bit each in the order Artifact, Battle, Creature, Enchantment, Instant, Kindred, Land, ...
const CREATURE = 1 << 2
const INSTANT = 1 << 4
const spell = (traits: Partial<PaySpend['traits']> = {}, has_x = false): PaySpend => ({
  kind: 'Spell',
  traits: { types: CREATURE, colors: 1, ...traits },
  has_x,
})
const ability: PaySpend = { kind: 'Ability', traits: { types: CREATURE, colors: 0 } }
const onSpell = (filter: SpendFilter): SpendOn => ({ Spell: filter })
const RIDER = { When: {} } as unknown as SpendOn

describe('spendAllows', () => {
  it.each<[string, PaySpend | undefined, SpendOn[] | undefined, boolean]>([
    ['no restriction', undefined, undefined, true],
    ['an empty list', spell(), [], true],
    ['a rider alone limits nothing', undefined, [RIDER], true],
    ['creature spells pay a creature spell', spell(), [onSpell({ types: ['Creature'] })], true],
    [
      'creature spells do not pay an instant',
      spell({ types: INSTANT }),
      [onSpell({ types: ['Creature'] })],
      false,
    ],
    ['creature spells do not pay an ability', ability, [onSpell({ types: ['Creature'] })], false],
    ['any spell pays a spell', spell({ types: INSTANT }), [onSpell({})], true],
    ['abilities pay an ability', ability, [{ Ability: {} }], true],
    ['abilities do not pay a spell', spell(), [{ Ability: {} }], false],
    ['a restriction pays nothing else', undefined, [onSpell({})], false],
    ['either use will do', ability, [onSpell({}), { Ability: {} }], true],
    ['a subtype', spell({ subtypes: ['Elf'] }), [onSpell({ subtypes: ['Elf', 'Goblin'] })], true],
    ['a missing subtype', spell({ subtypes: ['Human'] }), [onSpell({ subtypes: ['Elf'] })], false],
    ['without a type', spell({ types: CREATURE | INSTANT }), [onSpell({ without: ['Instant'] })], false],
    ['a colour of the spell', spell({ color_set: 1 << 4 }), [onSpell({ colors: ['Green'] })], true],
    ['another colour', spell({ color_set: 1 << 4 }), [onSpell({ colors: ['Red'] })], false],
    ['multicolored', spell({ colors: 2 }), [onSpell({ multicolored: true })], true],
    ['one colour is not multicolored', spell({ colors: 1 }), [onSpell({ multicolored: true })], false],
    ['mana value at least', spell({ value: 5 }), [onSpell({ min_value: 5 })], true],
    ['mana value too low', spell({ value: 4 }), [onSpell({ min_value: 5 })], false],
    ['a fact that holds', spell({ from_graveyard: true }), [onSpell({ facts: ['FromGraveyard'] })], true],
    ['a fact that does not', spell(), [onSpell({ facts: ['FromGraveyard'] })], false],
    ['an unresolved chosen type holds for nothing', spell(), [onSpell({ chosen_type: true })], false],
    ['a cost with X pays a spell with X', spell({}, true), ['CostWithX'], true],
    ['a cost with X pays an ability with X', { ...ability, has_x: true }, ['CostWithX'], true],
    ['a cost without X is not paid by it', spell(), ['CostWithX'], false],
  ])('%s', (_name, spend, only, expected) => {
    expect(spendAllows(spend, only)).toBe(expected)
  })

  it.each<[SpendOn[], string]>([
    [[onSpell({ types: ['Creature'] })], 'creature spells'],
    [[{ Ability: {} }], 'abilities'],
    [['CostWithX'], 'costs with {X}'],
    [[onSpell({ subtypes: ['Elf'] }), { Ability: {} }], 'elf spells or abilities'],
    [[RIDER], 'anything'],
  ])('words %j', (only, words) => {
    expect(usesWords(only)).toBe(words)
  })
})

const restricted = (letter: string, only: SpendOn[]): Restricted => ({
  mana: source(1, letter).produces,
  only,
})
const held = (...list: Restricted[]): ManaPool => ({ ...poolOf(''), restricted: list })
const spendOf = (index: number, one: Restricted): Spend => ({ kind: 'restricted', index, restricted: one })
const reachFor = (spend: PaySpend | undefined, sources: ManaOption[] = []): Reach => ({
  sources,
  maxLife: 0,
  spend,
})

describe('paying with restricted mana', () => {
  const creature = restricted('G', [onSpell({ types: ['Creature'] })])
  const pool = held(creature, creature)

  it.each<[string, PaySpend | undefined, boolean]>([
    ['a creature spell', spell(), true],
    ['an instant', spell({ types: INSTANT }), false],
    ['an ability', ability, false],
    ['neither', undefined, false],
  ])('restricted mana joins a payment for %s only when allowed', (_name, spend, expected) => {
    expect(allowed(cost('{G}'), pool, [], spendOf(0, creature), [], reachFor(spend))).toBe(expected)
  })

  it('uses each restricted mana of the pool once', () => {
    const reach = reachFor(spell())
    expect(allowed(cost('{G}{G}'), pool, [spendOf(0, creature)], spendOf(0, creature), [], reach)).toBe(false)
    expect(allowed(cost('{G}{G}'), pool, [spendOf(0, creature)], spendOf(1, creature), [], reach)).toBe(true)
  })

  it('covers a cost with restricted mana and sends it', () => {
    const spends = [spendOf(0, creature), spendOf(1, creature)]
    expect(covers(cost('{G}{G}'), spends)).toBe(true)
    expect(toPayment(spends)).toEqual({ pool: [], restricted: [creature, creature], taps: [], life: 0 })
  })

  it('leaves the restricted field out when none is spent', () => {
    expect(toPayment([])).toEqual({ pool: [], taps: [], life: 0 })
  })

  it('offers a source with restricted mana only for what it allows', () => {
    const treasure: ManaOption = { ...source(7, 'G'), only: [onSpell({ types: ['Creature'] })] }
    expect(usableByHand(treasure, spell())).toBe(true)
    expect(usableByHand(treasure, ability)).toBe(false)
    expect(usableByHand(treasure)).toBe(false)
    const tap: Spend = { kind: 'tap', option: treasure }
    expect(allowed(cost('{G}'), poolOf(''), [], tap, [], reachFor(spell(), [treasure]))).toBe(true)
    expect(allowed(cost('{G}'), poolOf(''), [], tap, [], reachFor(ability, [treasure]))).toBe(false)
  })

  it('finishes a payment from the restricted mana of the pool', () => {
    const reach: Reach = { ...reachFor(spell()), extras: [spendOf(0, creature)] }
    expect(allowed(cost('{G}'), pool, [], { kind: 'pool', mana: 'Green' }, [], reach)).toBe(false)
    expect(allowed(cost('{G}'), pool, [], spendOf(0, creature), [], reach)).toBe(true)
  })
})
