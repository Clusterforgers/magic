/** Goal: the client's idea of a valid payment is exactly the engine's, for plain, hybrid and Phyrexian costs: for many costs and choices of life, a payment can be built from fitting additions in any order if and only if a symbol-by-symbol check accepts it, what is owed is the cost minus what is covered (a hybrid symbol stays hybrid until mana meets it), life is only paid within the life total and when the mana chosen still fits, and a permanent or floating mana is never used twice; sources that make several mana, cost mana (paid only from mana made before them), sacrifice or make restricted mana are read as the engine reads them, a source is offered only while the payment can still be finished, and taking a pick back drops the sources it paid for. */
import { describe, expect, it } from 'vitest'
import type { ManaCost, ManaOption, ManaType } from '@/protocol/generated'
import { cost, manaOfLetter, poolOf, source } from '@/test-utils/mana'
import { shapeOf } from '../manaSymbols'
import {
  completable,
  withoutSpend,
  allowed,
  covers,
  fits,
  fittingOptions,
  lifeChoices,
  lifeOf,
  manaValue,
  MANA_TYPES,
  owed,
  poolLeft,
  resolutions,
  toPayment,
} from './payment'
import type { LifeSymbols, Spend } from './payment'
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

describe('owed with hybrid and Phyrexian symbols', () => {
  it.each([
    ['a hybrid symbol stays hybrid at the start', '{W/U}', [], [], '1 white or blue', false],
    ['and is met by either colour', '{W/U}', taps('W'), [], 'nothing', true],
    ['and by the other', '{W/U}', taps('U'), [], 'nothing', true],
    ['mana of neither colour is not met by it', '{W/U}', taps('G'), [], '1 white or blue', false],
    [
      'two equal hybrid symbols stay open after one is met',
      '{R/W}{R/W}',
      taps('R'),
      [],
      '1 red or white',
      false,
    ],
    ['and are met by either mix', '{R/W}{R/W}', taps('WR'), [], 'nothing', true],
    ['a plain symbol is met before a hybrid one', '{W}{W/U}', taps('W'), [], '1 white or blue', false],
    ['and the hybrid one by the colour left over', '{W}{W/U}', taps('UW'), [], 'nothing', true],
    ['a monocolored hybrid symbol is met by its colour', '{2/W}', taps('W'), [], 'nothing', true],
    ['and by two of anything', '{2/W}', taps('GG'), [], 'nothing', true],
    ['one of anything leaves one generic owed', '{2/W}', taps('G'), [], '1 generic', false],
    ['a Phyrexian symbol is met by its colour', '{W/P}', taps('W'), [], 'nothing', true],
    ['and stays open without it', '{W/P}', [], [], '1 white or 2 life', false],
    ['and by life when life pays', '{W/P}', [], [0], 'nothing', true],
    ['life leaves the other symbols', '{1}{W/P}{G/P}', taps('R'), [1], '1 green or 2 life', false],
    ['a hybrid Phyrexian symbol is met by either colour', '{G/U/P}', taps('U'), [], 'nothing', true],
    ['and by life', '{G/U/P}', [], [0], 'nothing', true],
    ['a colorless hybrid symbol is met by colorless', '{C/W}', taps('C'), [], 'nothing', true],
    ['and by its colour', '{C/W}', taps('W'), [], 'nothing', true],
    ['the colorless Phyrexian symbol is met by life', '{C/P}', [], [0], 'nothing', true],
    ['life that pays too much is not a covered payment', '{W/P}', taps('W'), [0], 'nothing', false],
  ] as [string, string, Spend[], number[], string, boolean][])(
    '%s: %s',
    (_what, text, spends, life, words, paid) => {
      expect(costWords(owed(cost(text), spends, life))).toBe(words)
      expect(covers(cost(text), spends, life)).toBe(paid)
    },
  )
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
    ['either hybrid colour', '{W/U}', '', 'U', true],
    ['not a third colour', '{W/U}', '', 'G', false],
    ['the second hybrid symbol can take the other colour', '{W/U}{W/U}', 'W', 'U', true],
    ['a monocolored hybrid symbol takes anything while it can be generic', '{2/W}', '', 'G', true],
    ['and a second thing', '{2/W}', 'G', 'R', true],
    ['but not a third', '{2/W}', 'GG', 'R', false],
    ['and a colour beside one generic, as the second of two generic', '{2/W}', 'G', 'W', true],
    ['a Phyrexian symbol takes its colour', '{W/P}', '', 'W', true],
    ['but nothing else', '{W/P}', '', 'G', false],
  ] as [string, string, string, string, boolean][])('%s', (_what, text, have, mana, expected) => {
    expect(fits(cost(text), taps(have), manaOfLetter(mana))).toBe(expected)
  })

  it('does not fit mana beside life that has paid for the symbol', () => {
    expect(fits(cost('{W/P}'), [], manaOfLetter('W'), [0])).toBe(false)
    expect(fits(cost('{1}{W/P}'), [], manaOfLetter('G'), [1])).toBe(true)
    expect(fits(cost('{1}{W/P}'), taps('G'), manaOfLetter('G'), [1])).toBe(false)
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

/** Takes one `mana` out of `spent`, or `undefined` when there is none. */
function without(spent: readonly ManaType[], mana: ManaType): ManaType[] | undefined {
  const at = spent.indexOf(mana)
  return at < 0 ? undefined : [...spent.slice(0, at), ...spent.slice(at + 1)]
}

/**
 * The independent check, symbol by symbol and unit by unit: does `spent` pay exactly for the symbols from `from` on,
 * the ones in `life` with life and the others with mana, in any way their rules allow?
 */
function pays(
  paying: ManaCost,
  from: number,
  spent: readonly ManaType[],
  life: ReadonlySet<number>,
): boolean {
  const symbol = paying[from]
  if (symbol === undefined) return spent.length === 0
  const next = (left: readonly ManaType[] | undefined): boolean =>
    left !== undefined && pays(paying, from + 1, left, life)
  const shape = shapeOf(symbol)
  if (life.has(from)) {
    return ['phyrexian', 'hybridPhyrexian', 'colorlessPhyrexian'].includes(shape.kind) && next(spent)
  }
  const one = (mana: ManaType): boolean => next(without(spent, mana))
  /** `count` units of anything, tried in every way, then the rest of the cost. */
  const anyOf = (count: number, left: readonly ManaType[]): boolean =>
    count === 0
      ? next(left)
      : MANA_TYPES.some((mana) => {
          const rest = without(left, mana)
          return rest !== undefined && anyOf(count - 1, rest)
        })
  switch (shape.kind) {
    case 'generic':
      return anyOf(shape.count, spent)
    case 'colored':
      return one(shape.color)
    case 'colorless':
      return one('Colorless')
    case 'hybrid':
      return shape.colors.some((color) => one(color))
    case 'monoHybrid':
      return one(shape.color) || anyOf(2, spent)
    case 'phyrexian':
      return one(shape.color)
    case 'hybridPhyrexian':
      return shape.colors.some((color) => one(color))
    case 'colorlessHybrid':
      return one(shape.color) || one('Colorless')
    case 'colorlessPhyrexian':
      return one('Colorless')
  }
}

/** Every set of symbol places (the symbols that can be paid with life) of a cost. */
function lifeChoicesOf(paying: ManaCost): number[][] {
  const capable = paying.flatMap((symbol, index) =>
    ['phyrexian', 'hybridPhyrexian', 'colorlessPhyrexian'].includes(shapeOf(symbol).kind) ? [index] : [],
  )
  return capable.reduce<number[][]>((sets, index) => [...sets, ...sets.map((set) => [...set, index])], [[]])
}

describe('the independent check', () => {
  it.each([
    ['{W/U}', 'U', [], true],
    ['{W/U}', 'G', [], false],
    ['{2/W}', 'GG', [], true],
    ['{2/W}', 'W', [], true],
    ['{2/W}', 'G', [], false],
    ['{W/P}', '', [0], true],
    ['{W/P}', 'W', [0], false],
    ['{G}', '', [0], false],
    ['{C/W}', 'C', [], true],
    ['{1}{B/P}{B/P}', 'G', [1, 2], true],
    ['{1}{B/P}{B/P}', 'BG', [2], true],
  ] as [string, string, number[], boolean][])(
    'knows %s with %s and life for %j: %s',
    (text, mana, life, expected) => {
      const spent = [...mana].map(manaOfLetter)
      expect(pays(cost(text), 0, spent, new Set(life))).toBe(expected)
    },
  )
})

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
    '{W/U}',
    '{W/U}{W/U}',
    '{R/W}{R/W}{R/W}',
    '{1}{G/W}{G/W}',
    '{U/B}{B}',
    '{W/B}{U/R}{G}',
    '{2/W}',
    '{1}{2/W}',
    '{2/W}{2/U}',
    '{2/W}{W}',
    '{W/P}',
    '{1}{W/P}',
    '{B/P}{B/P}',
    '{1}{B/P}{B/P}',
    '{G/U/P}',
    '{G/U/P}{G}',
    '{C/W}',
    '{C/W}{W}',
    '{C/P}',
    '{C/P}{C}',
    '{2}{W/P}{G/U/P}',
    '{R/W}{2/G}{B/P}',
  ])(
    '%s: adding fitting mana in any order reaches a payment the symbol check accepts, and only those',
    (text) => {
      const paying = cost(text)
      // Hybrid and Phyrexian costs take anything from nothing up to their mana value; a plain one takes exactly that.
      const total = Math.min(manaValue(paying), 4)
      for (const life of lifeChoicesOf(paying)) {
        const paid = new Set(life)
        for (let size = 0; size <= total; size += 1) {
          for (const mana of multisets(size)) {
            const accepted = pays(paying, 0, mana, paid)
            for (const order of orders(mana)) {
              const spends: Spend[] = []
              let every = true
              for (const next of order) {
                if (!fits(paying, spends, next, life)) every = false
                if (!every) break
                spends.push({ kind: 'pool', mana: next })
              }
              const reached = every && covers(paying, spends, life)
              expect(reached, `${order.join(',')} for ${text} with life ${JSON.stringify(life)}`).toBe(
                accepted,
              )
            }
          }
        }
      }
    },
  )

  it('never lets a payment grow past the cost', () => {
    const paying = cost('{1}{G}')
    for (const mana of multisets(2)) {
      const spends = mana.map((type): Spend => ({ kind: 'pool', mana: type }))
      for (const extra of MANA_TYPES) expect(fits(paying, spends, extra)).toBe(false)
    }
  })

  it.each(['{R/W}{R/W}', '{2/W}', '{W/P}{G/P}', '{1}{G/U/P}'])(
    '%s: a covered payment takes no more mana, with or without life',
    (text) => {
      const paying = cost(text)
      for (const life of lifeChoicesOf(paying)) {
        for (const mana of multisets(manaValue(paying))) {
          const spends = mana.map((type): Spend => ({ kind: 'pool', mana: type }))
          if (!covers(paying, spends, life)) continue
          for (const extra of MANA_TYPES) expect(fits(paying, spends, extra, life)).toBe(false)
        }
      }
    },
  )
})

describe('the choices of a cost', () => {
  it.each([
    ['a plain cost has one', '{2}{G}', 1],
    ['a free cost has one', '', 1],
    ['two equal hybrid symbols split between two ways', '{R/W}{R/W}', 3],
    ['three of them', '{R/W}{R/W}{R/W}', 4],
    ['a monocolored hybrid symbol', '{2/W}', 2],
    ['five of them', '{2/W}{2/U}{2/B}{2/R}{2/G}', 32],
    ['a Phyrexian symbol without its life', '{W/P}', 1],
    ['a hybrid Phyrexian symbol without its life', '{G/U/P}', 2],
    ['two equal Phyrexian symbols without their life', '{1}{B/P}{B/P}', 1],
    ['a colorless hybrid symbol', '{C/W}', 2],
  ])('%s: %s has %i without life', (_what, text, count) => {
    expect(resolutions(cost(text))).toHaveLength(count)
  })

  it('splits the life out: symbols that life pays leave no mana to pay', () => {
    expect(resolutions(cost('{1}{W/P}{G/P}'), [1, 2])).toEqual([[{ need: null, origin: 0 }]])
    expect(resolutions(cost('{1}{W/P}{G/P}'), [1])).toEqual([
      [
        { need: null, origin: 0 },
        { need: 'Green', origin: 2 },
      ],
    ])
  })

  it('gives each unit the place of the symbol it came from', () => {
    const [first] = resolutions(cost('{2}{R/W}'))
    expect(first).toEqual([
      { need: null, origin: 0 },
      { need: null, origin: 0 },
      { need: 'Red', origin: 1 },
    ])
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

  it('offers the colours of a hybrid symbol and nothing beside it', () => {
    const hybrid = cost('{W/U}')
    const abilities = [tap(1, 'W', 0), tap(1, 'U', 1), tap(1, 'G', 2)]
    expect(fittingOptions(hybrid, poolOf(''), [], abilities).map((o) => o.produces)).toEqual([
      'White',
      'Blue',
    ])
  })

  it('offers nothing to a symbol that life has paid for', () => {
    const abilities = [tap(1, 'W', 0)]
    expect(fittingOptions(cost('{W/P}'), poolOf(''), [], abilities, [0])).toEqual([])
    expect(fittingOptions(cost('{W/P}'), poolOf(''), [], abilities)).toHaveLength(1)
  })
})

describe('paying life for Phyrexian symbols', () => {
  const dismember = cost('{1}{B/P}{B/P}')

  it('lists a switch for each symbol that life can pay, and none for the others', () => {
    expect(lifeChoices(dismember, [], [], 4).map((c) => c.index)).toEqual([1, 2])
    expect(lifeChoices(cost('{2}{W}'), [], [], 20)).toEqual([])
    expect(lifeChoices(cost('{W/U}{2/W}'), [], [], 20)).toEqual([])
    expect(lifeChoices(cost('{C/P}{G/U/P}'), [], [], 4).map((c) => c.index)).toEqual([0, 1])
  })

  it.each([
    ['with life for both', 4, [], [true, true]],
    ['with life for one', 2, [], [true, true]],
    ['with life for none', 0, [], [false, false]],
    ['with one symbol paid and life for one', 2, [1], [true, false]],
    ['with both paid, and room to take either back', 4, [1, 2], [true, true]],
  ] as [string, number, number[], boolean[]][])(
    '%s (at most %i): which switches work',
    (_what, maxLife, paid, work) => {
      const choices = lifeChoices(dismember, [], paid, maxLife)
      expect(choices.map((choice) => choice.available)).toEqual(work)
      expect(choices.map((choice) => choice.paid)).toEqual([paid.includes(1), paid.includes(2)])
    },
  )

  it('refuses life that mana already chosen has no place beside', () => {
    // {1}{W/P} with a Plains and a Forest chosen: life for the symbol would leave one of them without a place.
    const choices = lifeChoices(cost('{1}{W/P}'), taps('WG'), [], 2)
    expect(choices.map((choice) => [choice.paid, choice.available])).toEqual([[false, false]])
    // With one of them taken back the switch works again.
    expect(lifeChoices(cost('{1}{W/P}'), taps('G'), [], 2).map((choice) => choice.available)).toEqual([true])
  })

  it('always lets life be taken back: a symbol paid with mana only adds room for mana', () => {
    expect(
      lifeChoices(cost('{1}{W/P}'), taps('G'), [1], 2).map((choice) => [choice.paid, choice.available]),
    ).toEqual([[true, true]])
    expect(lifeChoices(cost('{W/P}{G/P}'), [], [0, 1], 4).map((choice) => choice.available)).toEqual([
      true,
      true,
    ])
  })

  it('counts the life as 2 for each symbol', () => {
    expect(lifeOf([])).toBe(0)
    expect(lifeOf([1])).toBe(2)
    expect(lifeOf([1, 2])).toBe(4)
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
      life: 0,
    })
  })

  it('is empty for no payment', () => {
    expect(toPayment([])).toEqual({ pool: [], taps: [], life: 0 })
  })

  it.each<[LifeSymbols, number]>([
    [[], 0],
    [[2], 2],
    [[1, 2], 4],
  ])('pays 2 life for each symbol life pays: %j is %i', (life, paid) => {
    expect(toPayment(taps('G'), life).life).toBe(paid)
  })
})

/** Sources as precons have them, each on a permanent of its own id. */
const SOURCES = {
  plains: (id: number): ManaOption => source(id, 'W'),
  island: (id: number): ManaOption => source(id, 'U'),
  swamp: (id: number): ManaOption => source(id, 'B'),
  mountain: (id: number): ManaOption => source(id, 'R'),
  forest: (id: number): ManaOption => source(id, 'G'),
  solRing: (id: number): ManaOption => ({ ...source(id, 'C'), count: 2 }),
  /** Orzhov Signet: {1}, {T}: add {W}{B}. */
  signet: (id: number): ManaOption => ({ ...source(id, 'W'), also: ['Black'], cost: cost('{1}') }),
  /** Mystic Gate's {W/U}, {T}: add {W}{W}. */
  gate: (id: number): ManaOption => ({ ...source(id, 'W'), count: 2, cost: cost('{W/U}') }),
  /** Graven Cairns's {B/R}, {T}: add {B}{B}. */
  cairns: (id: number): ManaOption => ({ ...source(id, 'B'), count: 2, cost: cost('{B/R}') }),
  /** A Prism's {1}, {T}: add one white. */
  prism: (id: number): ManaOption => ({ ...source(id, 'W'), cost: cost('{1}') }),
  /** Orzhov Basilica: {T}: add {W}{B}. */
  basilica: (id: number): ManaOption => ({ ...source(id, 'W'), also: ['Black'] }),
  treasure: (id: number): ManaOption => ({ ...source(id, 'B'), sacrifice: true }),
  /** Mishra's Workshop: mana only for artifact spells. */
  workshop: (id: number): ManaOption => ({
    ...source(id, 'C'),
    count: 3,
    only: [{ Spell: { types: ['Artifact'] } }],
  }),
} satisfies Record<string, (id: number) => ManaOption>
type SourceName = keyof typeof SOURCES

/** The sources named, on permanents 1, 2, 3 and on, in that order. */
const sourcesOf = (names: readonly SourceName[]): ManaOption[] =>
  names.map((name, at) => SOURCES[name](at + 1))
const tapsOf = (names: readonly SourceName[]): Spend[] =>
  sourcesOf(names).map((option) => ({ kind: 'tap', option }))
const only = <T>(value: T | undefined): T => {
  if (value === undefined) throw new Error('a test names a source it does not list')
  return value
}

describe('sources that make several mana or cost mana', () => {
  it.each<[string, string, SourceName[], LifeSymbols, boolean]>([
    ['Sol Ring pays two generic', '{2}', ['solRing'], [], true],
    ['Sol Ring overpays one generic', '{1}', ['solRing'], [], false],
    ['Sol Ring and a land pay three', '{3}', ['solRing', 'forest'], [], true],
    ['Sol Ring and two lands overpay three', '{3}', ['solRing', 'forest', 'plains'], [], false],
    ['a Signet paid by a land', '{W}{B}', ['plains', 'signet'], [], true],
    ['a Signet listed before its land', '{W}{B}', ['signet', 'plains'], [], true],
    ['a Signet with nothing to pay it', '{W}{B}', ['signet'], [], false],
    ['a Signet and its land are two mana, not three', '{1}{W}{B}', ['plains', 'signet'], [], false],
    ['a Signet, its land and one more', '{1}{W}{B}', ['plains', 'signet', 'swamp'], [], true],
    ['a filter land paid by an Island', '{W}{W}', ['island', 'gate'], [], true],
    ['a filter land cannot take red', '{W}{W}', ['mountain', 'gate'], [], false],
    ['a filter land turns red into black', '{B}{B}', ['mountain', 'cairns'], [], true],
    ['a bounceland pays two colours', '{W}{B}', ['basilica'], [], true],
    ['a bounceland pays a colour and generic', '{1}{W}', ['basilica'], [], true],
    ['a bounceland overpays one colour', '{W}', ['basilica'], [], false],
    ['a Treasure pays its colour', '{B}', ['treasure'], [], true],
    ['hybrid from a Signet', '{W/B}{W/B}', ['swamp', 'signet'], [], true],
    ['hybrid from a filter land', '{W/U}{W/U}', ['island', 'gate'], [], true],
    ['Phyrexian with life and Sol Ring', '{2}{W/P}', ['solRing'], [1], true],
    ['Phyrexian with life and a Signet overpays', '{1}{W/P}', ['plains', 'signet'], [1], false],
    ['Phyrexian paid by a Signet', '{B}{W/P}', ['swamp', 'signet'], [], true],
    ['a Prism that could be left out is spare', '{1}', ['forest', 'prism'], [], false],
    ['a Prism that changes the colour is needed', '{W}', ['forest', 'prism'], [], true],
  ])('%s: %s covered by %j is %s', (_what, text, names, life, expected) => {
    expect(covers(cost(text), tapsOf(names), life)).toBe(expected)
  })

  it.each<[string, string, SourceName[], SourceName[], SourceName, boolean]>([
    ['a Signet before the mana that pays it', '{W}{B}', [], ['plains', 'signet'], 'signet', false],
    ['a Signet after its land', '{W}{B}', ['plains'], ['plains', 'signet'], 'signet', true],
    ['Sol Ring for one generic', '{1}', [], ['solRing', 'forest'], 'solRing', false],
    ['Sol Ring for two generic', '{2}', [], ['solRing', 'forest'], 'solRing', true],
    ['a land after Sol Ring paid two', '{2}', ['solRing'], ['solRing', 'forest'], 'forest', false],
    ['restricted mana is not offered by hand', '{3}', [], ['workshop'], 'workshop', false],
    ['an Island that leaves a blue no one makes', '{U}{U}', [], ['island', 'forest'], 'island', false],
    ['red that only a filter land can take', '{B}{B}', [], ['mountain', 'cairns'], 'mountain', true],
    ['red with no filter land to take it', '{B}{B}', [], ['mountain', 'swamp'], 'mountain', false],
    ['the filter land after the red', '{B}{B}', ['mountain'], ['mountain', 'cairns'], 'cairns', true],
    ['a Treasure when a land would do', '{B}', [], ['swamp', 'treasure'], 'treasure', true],
  ])('%s: %s, chosen %j of %j, %s offered is %s', (_what, text, chosen, all, next, expected) => {
    const options = sourcesOf(all)
    const spends: Spend[] = chosen.map((name) => ({ kind: 'tap', option: only(options[all.indexOf(name)]) }))
    const option = only(options[all.indexOf(next)])
    const reach = { sources: options, maxLife: 0 }
    expect(allowed(cost(text), poolOf(''), spends, { kind: 'tap', option }, [], reach)).toBe(expected)
  })

  it.each<[string, string, SourceName[], string, number, boolean]>([
    ['enough lands', '{1}{W}', ['plains', 'forest'], '', 0, true],
    ['too little mana', '{3}', ['plains', 'forest'], '', 0, false],
    ['floating mana finishes it', '{3}', ['plains', 'forest'], 'R', 0, true],
    ['life finishes a Phyrexian symbol', '{1}{B/P}', ['forest'], '', 2, true],
    ['life the payment may not take', '{1}{B/P}', ['forest'], '', 0, false],
    ['a Signet chain', '{1}{W}{B}', ['plains', 'signet', 'swamp'], '', 0, true],
    ['only restricted mana', '{3}', ['workshop'], '', 0, false],
  ])('completable: %s (%s from %j, pool %s, %i life)', (_what, text, names, floating, maxLife, expected) => {
    const reach = { sources: sourcesOf(names), maxLife }
    expect(completable(cost(text), poolOf(floating), [], [], reach)).toBe(expected)
  })

  it.each<[string, SourceName[], number, number[]]>([
    ['taking back the land drops the Signet it paid for', ['plains', 'signet', 'swamp'], 0, [3]],
    ['taking back the Signet keeps the lands', ['plains', 'signet', 'swamp'], 1, [1, 3]],
    ['taking back a land the Signet does not need', ['plains', 'swamp', 'signet'], 1, [1, 3]],
  ])('%s', (_what, names, index, kept) => {
    const left = withoutSpend(tapsOf(names), index)
    expect(left.map((spend) => (spend.kind === 'tap' ? spend.option.source.object : 0))).toEqual(kept)
  })
})
