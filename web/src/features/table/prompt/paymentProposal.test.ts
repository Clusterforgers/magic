/** Goal: the game's payment ("Pay for me" proposes it) is read back into the pieces the prompt shows for many costs and sources (generic, coloured, a land of two colours whose kind is read from what the cost needs, floating mana, life for a Phyrexian symbol), the pieces cover the cost and send the same payment back, an unreadable payment gives nothing, and "Pay the rest for me" finishes the picks with the game's sources first or finds that nothing can. */
import { describe, expect, it } from 'vitest'
import type { ManaOption, Payment } from '@/protocol/generated'
import { cost, manaOfLetter, poolOf, source } from '@/test-utils/mana'
import { completion, covers, fromPayment, toPayment } from './payment'
import type { Spend } from './payment'

/** A payment of floating mana `floating` (letters), taps `[object, ability]` and `life`. */
const paid = (floating: string, taps: [number, number][], life = 0): Payment => ({
  pool: [...floating].map(manaOfLetter),
  taps: taps.map(([object, ability]) => ({ object, ability })),
  life,
})

/** A land of two colours: one ability, either kind. */
const dual = (object: number, letters: string): ManaOption[] =>
  [...letters].map((letter) => source(object, letter))

const PROPOSALS: [string, string, string, ManaOption[], Payment, string[]][] = [
  [
    'generic from two lands',
    '{2}',
    '',
    [source(1, 'G'), source(2, 'U')],
    paid('', [
      [1, 0],
      [2, 0],
    ]),
    ['G', 'U'],
  ],
  [
    'a land of two colours made to pay the colour left',
    '{W}{U}',
    '',
    [...dual(1, 'WU'), source(2, 'W')],
    paid('', [
      [1, 0],
      [2, 0],
    ]),
    ['U', 'W'],
  ],
  ['floating mana first', '{1}{G}', 'R', [source(1, 'G')], paid('R', [[1, 0]]), ['R', 'G']],
  ['life for a Phyrexian symbol', '{1}{W/P}', '', [source(1, 'G')], paid('', [[1, 0]], 2), ['G']],
]

describe('fromPayment', () => {
  it.each(PROPOSALS)('reads %s', (_what, text, floating, sources, payment, made) => {
    const whole = fromPayment(payment, cost(text), poolOf(floating), sources)
    expect(whole).toBeDefined()
    if (whole === undefined) return
    expect(covers(cost(text), whole.spends, whole.life)).toBe(true)
    expect(toPayment(whole.spends, whole.life)).toEqual(payment)
    const kinds = whole.spends.map((spend) =>
      spend.kind === 'pool' ? spend.mana : spend.kind === 'tap' ? spend.option.produces : spend.kind,
    )
    expect(kinds).toEqual(made.map(manaOfLetter))
  })

  it.each([
    ['a source the decision does not offer', '{1}', [source(1, 'G')], paid('', [[9, 0]])],
    ['too little for the cost', '{2}', [source(1, 'G')], paid('', [[1, 0]])],
    ['a colour no source makes', '{U}', [source(1, 'G')], paid('', [[1, 0]])],
  ])('gives nothing for %s', (_what, text, sources, payment) => {
    expect(fromPayment(payment, cost(text), poolOf(''), sources)).toBeUndefined()
  })
})

describe('completion', () => {
  const sources = [source(1, 'G'), source(2, 'U'), source(3, 'B'), source(4, 'W')]
  const picked: Spend[] = [{ kind: 'tap', option: source(1, 'G') }]

  it.each([
    ['the game prefers land 3', [3], 3],
    ['the game prefers land 4', [4], 4],
  ])('keeps the picks and adds what is owed, %s', (_what, prefer, expected) => {
    const whole = completion(cost('{1}{G}'), poolOf(''), picked, [], { sources, maxLife: 0 }, (option) =>
      prefer.includes(option.source.object),
    )
    expect(whole?.spends[0]).toEqual(picked[0])
    expect(whole === undefined ? false : covers(cost('{1}{G}'), whole.spends, whole.life)).toBe(true)
    expect(whole?.spends.map((spend) => (spend.kind === 'tap' ? spend.option.source.object : 0))).toEqual([
      1,
      expected,
    ])
  })

  it.each([
    ['a colour no source makes', '{R}', []],
    ['more than the sources make', '{5}', picked],
  ])('finds nothing for %s', (_what, text, given) => {
    expect(completion(cost(text), poolOf(''), given, [], { sources, maxLife: 0 })).toBeUndefined()
  })
})
