/** Goal: what a source gives is read the same from a payment option (Sol Ring, a basic land, a source of any colour, a bounce land, a Signet, a Treasure, a choice of two colours) and from the words of an activated ability, and a card shows a tap's mana only while the seat may tap it. */
import { describe, expect, it } from 'vitest'
import type { ManaOption, ManaType, ObjectView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { cost, manaOfLetter } from '@/test-utils/mana'
import { manaLetter } from './cardModel'
import { buildFixture } from './dev/fixture'
import { givesNow, givesOfOptions, manaAbilitiesOf, madeCost } from './manaMade'

const option = (letter: string, extra: Partial<ManaOption> = {}, ability = 0): ManaOption => ({
  source: { object: 1, ability },
  produces: manaOfLetter(letter),
  ...extra,
})
const anyOf = (extra: Partial<ManaOption> = {}): ManaOption[] =>
  [...'WUBRG'].map((letter, ability) => option(letter, extra, ability))
const letters = (made: readonly ManaType[]): string => made.map(manaLetter).join('')

describe('givesOfOptions', () => {
  it.each<[string, ManaOption[], { any: boolean; ways: string[]; cost: string; sacrifice: boolean }]>([
    ['a basic land is one mana', [option('G')], { any: false, ways: ['G'], cost: '', sacrifice: false }],
    [
      'Sol Ring is two colourless',
      [option('C', { count: 2 })],
      { any: false, ways: ['CC'], cost: '', sacrifice: false },
    ],
    [
      'a bounce land is both of its mana',
      [option('W', { also: ['Blue'] })],
      { any: false, ways: ['WU'], cost: '', sacrifice: false },
    ],
    [
      'a Signet is its cost, then both mana',
      [option('W', { also: ['Black'], cost: cost('{1}') })],
      { any: false, ways: ['WB'], cost: '{1}', sacrifice: false },
    ],
    [
      'a source with an option for each colour is any colour',
      anyOf(),
      { any: true, ways: [], cost: '', sacrifice: false },
    ],
    [
      'a Treasure is any colour and is sacrificed',
      anyOf({ sacrifice: true }),
      { any: true, ways: [], cost: '', sacrifice: true },
    ],
    [
      'two colours are listed, not any',
      [option('R', {}, 0), option('W', {}, 1)],
      { any: false, ways: ['R', 'W'], cost: '', sacrifice: false },
    ],
    [
      'four colours are listed, not any',
      [...'WUBR'].map((letter, at) => option(letter, {}, at)),
      { any: false, ways: ['W', 'U', 'B', 'R'], cost: '', sacrifice: false },
    ],
    [
      'two abilities of one colour are one way',
      [option('G', {}, 0), option('G', {}, 1)],
      { any: false, ways: ['G'], cost: '', sacrifice: false },
    ],
  ])('%s', (_name, options, expected) => {
    const gives = givesOfOptions(options)
    expect(gives.any).toBe(expected.any)
    expect(gives.ways.map((way) => letters(way.made))).toEqual(expected.ways)
    expect(gives.cost).toEqual(cost(expected.cost))
    expect(gives.sacrifice).toBe(expected.sacrifice)
  })

  it('writes mana as the symbols of a cost', () => {
    expect(madeCost(['Colorless', 'Colorless', 'Green'])).toEqual(cost('{C}{C}{G}'))
  })
})

const fixture = buildFixture({ seats: 2 })
const land = must(
  fixture.view.objects.find(
    (object) => object.zone === 'Battlefield' && object.controller === 0 && object.types.includes('Land'),
  ),
)
const tapAbility = (cost: string, effect: string) => ({ cost, effect, mana: true })
const permanent = (change: Partial<ObjectView>): ObjectView => ({
  ...land,
  permanent: { tapped: false, damage: 0, summoning_sick: false },
  ...change,
})

describe('manaAbilitiesOf reads the words of an activated ability', () => {
  it.each([
    ['{T}', 'Add {G}.', 'G', false],
    ['{T}', 'Add {C}{C}.', 'CC', false],
    ['{T}', 'Add {W} or {U}.', 'W/U', false],
    ['{T}', 'Add {W}{U}.', 'WU', false],
    ['{1}, {T}', 'Add {W}{B}.', 'WB', false],
    ['{T}', 'Add one mana of any color.', 'any', false],
    ['{T}, Sacrifice this artifact', 'Add one mana of any color.', 'any', true],
  ])('%s: %s', (costWords, effect, shown, sacrifice) => {
    const [ability] = manaAbilitiesOf(permanent({ activated: [tapAbility(costWords, effect)] }))
    const gives = must(ability).gives
    expect(gives.any ? 'any' : gives.ways.map((way) => letters(way.made)).join('/')).toBe(shown)
    expect(gives.sacrifice).toBe(sacrifice)
    expect(must(ability).taps).toBe(costWords.includes('{T}'))
  })

  it.each([
    ['a cost it cannot read', '{T}, Pay 1 life', 'Add {G}.'],
    [
      'an effect that does more than add mana at once',
      '{T}',
      'Add {G}. If you control four creatures, add {G}{G} instead.',
    ],
    ['an effect that is no mana', '{T}', 'Draw a card.'],
  ])('leaves out %s', (_name, costWords, effect) => {
    expect(manaAbilitiesOf(permanent({ activated: [tapAbility(costWords, effect)] }))).toEqual([])
  })

  it('leaves out an ability that is not a mana ability', () => {
    expect(manaAbilitiesOf(permanent({ activated: [{ cost: '{T}', effect: 'Add {G}.' }] }))).toEqual([])
  })

  it('falls back to the printed abilities while the view lists no activated ones', () => {
    const [ability] = manaAbilitiesOf(permanent({ abilities: [{ Mana: { produces: 'Red' } }] }))
    expect(must(ability).gives.ways.map((way) => letters(way.made))).toEqual(['R'])
  })
})

describe('givesNow shows a tap only while the seat may tap it', () => {
  const ring = { activated: [tapAbility('{T}', 'Add {C}{C}.')] }
  const tapped = { tapped: true, damage: 0, summoning_sick: false }
  const sick = { tapped: false, damage: 0, summoning_sick: true }
  it.each<[string, Partial<ObjectView>, number | null, boolean]>([
    ['an untapped land of the seat', ring, 0, true],
    ['a tapped land', { ...ring, permanent: tapped }, 0, false],
    ["another seat's land", ring, 1, false],
    ['a viewer without a seat', ring, null, false],
    ['a land in the hand', { ...ring, zone: { Hand: 0 }, permanent: null }, 0, false],
    ['a face-down permanent', { ...ring, face_down: true }, 0, false],
    ['a creature that came this turn', { ...ring, types: ['Creature'], permanent: sick }, 0, false],
    ['a land that came this turn', { ...ring, permanent: sick }, 0, true],
    ['a land with no mana ability', { activated: [], abilities: [] }, 0, false],
    [
      'an ability that does not tap',
      { activated: [tapAbility('Sacrifice this artifact', 'Add {C}.')] },
      0,
      false,
    ],
  ])('%s', (_name, change, seat, shown) => {
    expect(givesNow(permanent({ controller: 0, ...change }), seat) !== null).toBe(shown)
  })

  it('folds two abilities into one set of ways, once each', () => {
    const gives = must(
      givesNow(
        permanent({
          activated: [
            tapAbility('{T}', 'Add {G}.'),
            tapAbility('{T}', 'Add {G}.'),
            tapAbility('{T}', 'Add {W}.'),
          ],
        }),
        0,
      ),
    )
    expect(gives.ways.map((way) => letters(way.made))).toEqual(['G', 'W'])
  })
})
