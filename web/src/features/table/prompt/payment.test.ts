/** Goal: the client's idea of a valid payment is exactly the engine's (total equal to the cost, every coloured and colourless requirement met): for many costs, a payment can be built from fitting additions in any order if and only if the engine would accept it, what is owed is the cost minus what is covered, and a permanent or floating mana is never used twice. */
import { describe, expect, it } from 'vitest'
import type { ManaCost, ManaType } from '@/protocol/generated'
import { cost, manaOfLetter, poolOf, source } from '@/test-utils/mana'
import {
  allowed,
  covers,
  fits,
  fittingOptions,
  manaValue,
  MANA_TYPES,
  owed,
  poolLeft,
  toPayment,
} from './payment'
import type { Spend } from './payment'
import { costWords } from './payText'

/** Floating mana from letters, such as `pool('WWG')`. */
const pool = (letters: string): Spend[] =>
  [...letters].map((letter) => ({ kind: 'pool', mana: manaOfLetter(letter) }))
/** Permanents tapped for mana, such as `taps('GU')`: object ids count up from 1. */
const taps = (letters: string): Spend[] =>
  [...letters].map((letter, index) => ({ kind: 'tap', option: source(index + 1, letter) }))
const tap = source

describe('owed and covers', () => {
  it.each([
    ['nothing chosen', '{2}{G}', [], '2 generic and 1 green', false],
    ['the colour first', '{2}{G}', taps('G'), '2 generic', false],
    ['generic first', '{2}{G}', taps('U'), '1 generic and 1 green', false],
    ['two generic and the colour', '{2}{G}', taps('UBG'), 'nothing', true],
    ['the colour spent as generic', '{1}{G}', taps('GG'), 'nothing', true],
    ['floating mana counts like a land', '{1}{G}', [...pool('G'), ...taps('R')], 'nothing', true],
    ['a wrong colour only pays generic', '{R}{R}', taps('R'), '1 red', false],
    ['colourless needs colourless', '{C}', taps('G'), '1 colorless', false],
    ['colourless paid', '{1}{C}', taps('GC'), 'nothing', true],
    ['five colours', '{W}{U}{B}{R}{G}', taps('WUBR'), '1 green', false],
    ['five colours paid', '{W}{U}{B}{R}{G}', taps('GRBUW'), 'nothing', true],
    ['a free spell', '', [], 'nothing', true],
    ['generic only', '{4}', taps('WUB'), '1 generic', false],
    ['generic only, paid', '{4}', taps('WUBG'), 'nothing', true],
  ] as [string, string, Spend[], string, boolean][])('%s: %s owes %s', (_what, text, spends, words, paid) => {
    expect(costWords(owed(cost(text), spends))).toBe(words)
    expect(covers(cost(text), spends)).toBe(paid)
  })

  it('does not count more mana than the cost as covered', () => {
    expect(covers(cost('{G}'), taps('GG'))).toBe(false)
    expect(covers(cost('{1}'), taps('GG'))).toBe(false)
  })
})

describe('what fits', () => {
  it.each([
    ['colour wanted', '{G}', '', 'G', true],
    ['wrong colour for a coloured cost', '{G}', '', 'R', false],
    ['anything for generic', '{2}', '', 'R', true],
    ['colour wanted after generic is full', '{1}{G}', 'U', 'U', false],
    ['the colour after generic is full', '{1}{G}', 'U', 'G', true],
    ['generic still open', '{1}{G}', 'G', 'U', true],
    ['generic full of the right colour', '{1}{G}', 'GG', 'G', false],
    ['colourless is not generic', '{C}', '', 'G', false],
    ['colourless wanted', '{C}', '', 'C', true],
    ['colourless as generic', '{1}', '', 'C', true],
    ['nothing fits a free spell', '', '', 'G', false],
    ['nothing fits a paid cost', '{G}', 'G', 'G', false],
  ] as [string, string, string, string, boolean][])('%s', (_what, text, have, mana, expected) => {
    expect(fits(cost(text), taps(have), manaOfLetter(mana))).toBe(expected)
  })
})

/** Every multiset of `size` kinds of mana. */
function multisets(size: number, from = 0): ManaType[][] {
  if (size === 0) return [[]]
  return MANA_TYPES.slice(from).flatMap((type, offset) =>
    multisets(size - 1, from + offset).map((rest) => [type, ...rest]),
  )
}
/** Every order of a list. */
function orders<T>(list: readonly T[]): T[][] {
  if (list.length <= 1) return [[...list]]
  return list.flatMap((item, index) =>
    orders([...list.slice(0, index), ...list.slice(index + 1)]).map((rest) => [item, ...rest]),
  )
}
/** The engine's rule: the total equals the cost's and every requirement is met. */
function engineAccepts(paying: ManaCost, mana: readonly ManaType[]): boolean {
  const need = [...paying.colored, paying.colorless]
  return (
    mana.length === manaValue(paying) &&
    MANA_TYPES.every((type, index) => mana.filter((m) => m === type).length >= (need[index] ?? 0))
  )
}

describe('agreement with the engine', () => {
  it.each([
    '',
    '{1}',
    '{3}',
    '{G}',
    '{C}',
    '{R}{R}',
    '{1}{G}',
    '{1}{C}',
    '{C}{C}',
    '{2}{G}',
    '{2}{W}{U}',
    '{1}{W}{B}',
    '{3}{B}{B}',
    '{G}{C}{R}',
    '{1}{C}{G}{G}',
    '{W}{U}{B}{R}',
  ])('%s: adding fitting mana in any order reaches a payment the engine accepts, and only those', (text) => {
    const paying = cost(text)
    const total = manaValue(paying)
    for (const mana of multisets(total)) {
      for (const order of orders(mana)) {
        const spends: Spend[] = []
        let every = true
        for (const next of order) {
          if (!fits(paying, spends, next)) every = false
          if (!every) break
          spends.push({ kind: 'pool', mana: next })
        }
        expect(every, `${order.join(',')} for ${text}`).toBe(engineAccepts(paying, mana))
        if (every) expect(covers(paying, spends)).toBe(true)
      }
    }
  })

  it('never lets a payment grow past the cost', () => {
    const paying = cost('{1}{G}')
    for (const mana of multisets(2)) {
      const spends = mana.map((type): Spend => ({ kind: 'pool', mana: type }))
      for (const extra of MANA_TYPES) expect(fits(paying, spends, extra)).toBe(false)
    }
  })
})

describe('what may join the payment', () => {
  const paying = cost('{1}{G}')

  it('lets a permanent be tapped once, even for its other ability', () => {
    const first = taps('G')
    expect(allowed(paying, poolOf(''), first, { kind: 'tap', option: tap(1, 'U', 1) })).toBe(false)
    expect(allowed(paying, poolOf(''), first, { kind: 'tap', option: tap(2, 'U') })).toBe(true)
  })

  it.each([
    ['none in the pool', '', '', 'G', 0, false],
    ['one in the pool', 'G', '', 'G', 1, true],
    ['one used already', 'G', 'G', 'G', 0, false],
    ['two, one used', 'UU', 'U', 'U', 1, true],
  ] as [string, string, string, string, number, boolean][])(
    'floating mana: %s',
    (_what, floating, used, mana, left, ok) => {
      const spends = pool(used)
      const kind = manaOfLetter(mana)
      expect(poolLeft(poolOf(floating), spends, kind)).toBe(left)
      // `{2}{U}{G}` leaves room for any of these once they are available.
      expect(allowed(cost('{2}{U}{G}'), poolOf(floating), spends, { kind: 'pool', mana: kind })).toBe(ok)
    },
  )

  it('offers one option per kind of mana from a permanent with several abilities', () => {
    const abilities = [tap(1, 'R', 0), tap(1, 'R', 1), tap(1, 'G', 2)]
    expect(fittingOptions(paying, poolOf(''), [], abilities).map((o) => o.source.ability)).toEqual([0, 2])
  })

  it('offers only the options that still fit', () => {
    const abilities = [tap(1, 'R'), tap(1, 'G', 1)]
    expect(fittingOptions(paying, poolOf(''), taps('UU').slice(1), abilities)).toHaveLength(1)
    expect(fittingOptions(paying, poolOf(''), [...taps('UB').slice(1), ...pool('G')], abilities)).toEqual([])
  })
})

describe('the payment on the wire', () => {
  it('splits floating mana from abilities, keeping each ability and the order', () => {
    const spends: Spend[] = [...pool('G'), ...taps('UB'), ...pool('RR')]
    expect(toPayment(spends)).toEqual({
      pool: ['Green', 'Red', 'Red'],
      taps: [
        { object: 1, ability: 0 },
        { object: 2, ability: 0 },
      ],
    })
  })

  it('is empty for no payment', () => {
    expect(toPayment([])).toEqual({ pool: [], taps: [] })
  })
})
