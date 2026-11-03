/** Goal: the colour switch offers exactly the other abilities of a chosen permanent that keep the payment finishable, usable by hand and funded in order, and a swap keeps every other piece in its place. */
import { describe, expect, it } from 'vitest'
import type { ManaOption, ManaType } from '@/protocol/generated'
import { cost, poolOf, source } from '@/test-utils/mana'
import { madeBy } from './payment'
import type { Reach, Spend } from './payment'
import { lastTapOf, swapOptions, withSwap } from './paySwap'

const LETTER_OF: Record<ManaType, string> = {
  White: 'W',
  Blue: 'U',
  Black: 'B',
  Red: 'R',
  Green: 'G',
  Colorless: 'C',
}

/** A permanent with one ability for each letter, abilities counted from 0. */
const abilities = (object: number, letters: string): ManaOption[] =>
  [...letters].map((letter, ability) => source(object, letter, ability))
const pick = (options: readonly ManaOption[], at: number): ManaOption => {
  const option = options[at]
  if (option === undefined) throw new Error('no such ability')
  return option
}
const tap = (options: readonly ManaOption[], at: number): Spend => ({
  kind: 'tap',
  option: pick(options, at),
})
/** A filter: `{1}` for two green. */
const filter = (object: number, ability: number): ManaOption => ({
  ...source(object, 'G', ability),
  count: 2,
  cost: [{ Generic: 1 }],
})
const ARTIFACT_ONLY: ManaOption['only'] = [{ Spell: { types: ['Artifact'] } }]

const tower = abilities(1, 'WUBRG')
const forest = abilities(2, 'G')

describe('swapOptions', () => {
  it.each<[string, string, ManaOption[], Spend[], number, ManaOption[], string[]]>([
    [
      'every colour that still finishes {1}{G}',
      '{1}{G}',
      [...tower, ...forest],
      [tap(tower, 1)],
      0,
      tower,
      ['W', 'B', 'R', 'G'],
    ],
    [
      'none where another colour would be wasted',
      '{G}{G}',
      [...tower, ...forest],
      [tap(forest, 0), tap(tower, 4)],
      1,
      tower,
      [],
    ],
    ['none once nothing else can make the green', '{1}{G}', tower, [tap(tower, 4)], 0, tower, []],
    ['the other colour of a hybrid symbol', '{W/U}', tower, [tap(tower, 0)], 0, tower, ['U']],
    ['nothing for floating mana', '{1}', tower, [{ kind: 'pool', mana: 'Green' }], 0, tower, []],
    ['nothing for a permanent of one ability', '{1}', forest, [tap(forest, 0)], 0, forest, []],
  ])('offers %s', (_what, text, sources, spends, index, own, expected) => {
    const reach: Reach = { sources, maxLife: 0 }
    const pool = poolOf(spends.some((spend) => spend.kind === 'pool') ? 'G' : '')
    const found = swapOptions(cost(text), pool, spends, index, own, [], reach)
    expect(
      found.map((option) =>
        madeBy(option)
          .map((mana) => LETTER_OF[mana])
          .join(''),
      ),
    ).toEqual(expected)
  })

  it('leaves out an ability that costs mana nothing before it pays for', () => {
    const own = [source(3, 'C', 0), filter(3, 1)]
    const reach: Reach = { sources: [...own, ...forest], maxLife: 0 }
    expect(swapOptions(cost('{2}'), poolOf(''), [tap(own, 0)], 0, own, [], reach)).toEqual([])
  })

  it('leaves out restricted mana the payment may not use', () => {
    const own = [source(4, 'C', 0), { ...source(4, 'R', 1), only: ARTIFACT_ONLY }]
    const reach: Reach = { sources: own, maxLife: 0 }
    expect(swapOptions(cost('{1}'), poolOf(''), [tap(own, 0)], 0, own, [], reach)).toEqual([])
  })
})

describe('withSwap and lastTapOf', () => {
  it('replaces the tap in its place and keeps the others', () => {
    const spends: Spend[] = [tap(forest, 0), tap(tower, 1), { kind: 'pool', mana: 'Red' }]
    const at = lastTapOf(spends, 1)
    expect(at).toBe(1)
    expect(withSwap(spends, at, pick(tower, 4))).toEqual([spends[0], tap(tower, 4), spends[2]])
    expect(lastTapOf(spends, 9)).toBe(-1)
  })
})
