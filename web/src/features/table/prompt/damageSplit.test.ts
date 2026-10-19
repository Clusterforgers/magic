/** Goal: for every power from 0 to 10 and every set of one to four blockers (lethal damage from 0 to 5, some already damaged), the default split adds up to the power, gives lethal damage in the offered order and the rest to the last, so it is always a legal answer; clamping never lets the total pass the power; and a split is judged legal exactly when it adds up to the power, with the reason in words otherwise. */
import { describe, expect, it } from 'vitest'
import {
  clampedAmount,
  defaultSplit,
  factsOf,
  hasDeathtouch,
  judge,
  labelled,
  lethalDamage,
  lethalEverywhere,
  lethalNote,
  restOf,
  setAmount,
  toAnswer,
  totalOf,
} from './damageSplit'
import type { DamageRecipient, Trample } from './damageSplit'

const blocker = (id: number, toughness: number | null, damage = 0): DamageRecipient => ({
  id,
  name: `Blocker ${id}`,
  toughness,
  damage,
})

const POWERS = Array.from({ length: 11 }, (_, power) => power)
// Toughness and damage already marked, so that lethal damage is 0 (twice), 1, 2, 3, 4 and 5.
const SHAPES: [number, number][] = [
  [3, 3],
  [2, 5],
  [2, 1],
  [2, 0],
  [3, 0],
  [4, 0],
  [5, 0],
]
const lineups = (size: number): DamageRecipient[][] =>
  size === 0
    ? [[]]
    : lineups(size - 1).flatMap((line) =>
        SHAPES.map(([toughness, damage]) => [...line, blocker(line.length + 1, toughness, damage)]),
      )
// Every lineup of one to three blockers, and a sample of four (all of them would be 2401).
const LINEUPS = [1, 2, 3].flatMap(lineups).concat([
  [blocker(1, 1), blocker(2, 1), blocker(3, 1), blocker(4, 1)],
  [blocker(1, 5), blocker(2, 5), blocker(3, 5), blocker(4, 5)],
  [blocker(1, 2, 2), blocker(2, 3, 1), blocker(3, 4), blocker(4, 1)],
  [blocker(1, 3), blocker(2, 2), blocker(3, 1), blocker(4, 4, 3)],
])
const show = (line: readonly DamageRecipient[]): string =>
  line.map(({ toughness, damage }) => `${toughness}/${damage}`).join(' ')
/** The lineups for which `holds` is false at `power`, written out so a failure names them. */
const failing = (
  power: number,
  holds: (split: number[], line: DamageRecipient[]) => boolean,
  lines = LINEUPS,
): string[] => lines.filter((line) => !holds(defaultSplit(power, line), line)).map(show)

describe('the default split', () => {
  it('covers every power from 0 to 10 over 403 lineups of one to four blockers', () => {
    expect(POWERS).toHaveLength(11)
    expect(LINEUPS).toHaveLength(7 + 49 + 343 + 4)
  })

  it.each(POWERS)(
    'of %i damage is a legal answer in whole numbers that adds up to the power, for every lineup',
    (power) => {
      expect(
        failing(power, (split, line) => {
          const whole = split.every((amount) => Number.isInteger(amount) && amount >= 0)
          return (
            whole && split.length === line.length && totalOf(split) === power && judge(power, split).legal
          )
        }),
      ).toEqual([])
    },
  )

  it.each(POWERS)(
    'of %i damage gives lethal damage in the offered order and the rest to the last, for every lineup',
    (power) => {
      expect(
        failing(power, (split, line) => {
          let left = power
          const front = line.slice(0, -1).map((recipient) => {
            const share = Math.min(left, lethalDamage(recipient) ?? 0)
            left -= share
            return share
          })
          return [...front, left].join() === split.join()
        }),
      ).toEqual([])
    },
  )

  it.each(POWERS)('of %i damage turns into an answer that the rules accept, for every lineup', (power) => {
    expect(
      failing(power, (split, line) => {
        const answer = toAnswer(line, split)
        const sent = 'AssignDamage' in answer ? answer.AssignDamage : []
        return totalOf(sent.map(([, amount]) => amount)) === power && sent.every(([, amount]) => amount > 0)
      }),
    ).toEqual([])
  })

  it.each([
    [5, [2, 2], [2, 3]],
    [4, [2, 2], [2, 2]],
    [1, [2, 2], [1, 0]],
    [0, [2, 2], [0, 0]],
    [6, [2, 3, 4], [2, 3, 1]],
    [10, [1, 1, 1, 1], [1, 1, 1, 7]],
    [3, [3], [3]],
    [9, [3], [9]],
  ])('splits %i damage over lethal %j as %j', (power, lethals, expected) => {
    const line = lethals.map((lethal, index) => blocker(index + 1, lethal))
    expect(defaultSplit(power, line)).toEqual(expected)
  })

  it('counts damage already marked towards lethal, and gives a creature that already has lethal damage nothing', () => {
    expect(defaultSplit(5, [blocker(1, 4, 3), blocker(2, 3, 3), blocker(3, 9)])).toEqual([1, 0, 4])
  })

  it('gives all of it to the last when a creature has no toughness to measure', () => {
    expect(defaultSplit(4, [blocker(1, null), blocker(2, 2)])).toEqual([0, 4])
  })

  it('gives nothing when there is nobody to give it to', () => {
    expect(defaultSplit(4, [])).toEqual([])
  })
})

describe('lethal damage', () => {
  it.each([
    [3, 0, 3],
    [3, 2, 1],
    [3, 3, 0],
    [3, 7, 0],
    [0, 0, 0],
  ])('of toughness %i with %i marked is %i', (toughness, damage, lethal) => {
    expect(lethalDamage(blocker(1, toughness, damage))).toBe(lethal)
  })

  it('is unknown without a toughness', () => {
    expect(lethalDamage(blocker(1, null))).toBeNull()
    expect(lethalNote(blocker(1, null), 3)).toBeNull()
    expect(factsOf(blocker(1, null))).toBe('No toughness')
  })

  it.each([
    [3, 0, 0, '3 short of lethal'],
    [3, 0, 2, '1 short of lethal'],
    [3, 0, 3, 'Lethal'],
    [3, 0, 8, 'Lethal'],
    [3, 1, 1, '1 short of lethal'],
    [3, 1, 2, 'Lethal'],
    [3, 3, 0, null],
    [3, 3, 2, null],
  ])('of toughness %i with %i marked and %i assigned is noted as %j', (toughness, damage, amount, note) => {
    expect(lethalNote(blocker(1, toughness, damage), amount)).toBe(note)
  })

  it.each([
    [4, 0, 'Toughness 4, lethal: 4'],
    [4, 1, 'Toughness 4, 1 damage marked, lethal: 3'],
    [2, 2, 'Toughness 2, 2 damage marked, lethal damage already marked'],
  ])('of toughness %i with %i marked is told as "%s"', (toughness, damage, words) => {
    expect(factsOf(blocker(1, toughness, damage))).toBe(words)
  })
})

describe('setting one amount', () => {
  it.each([
    ['inside what is left', [2, 1], 0, 3, 5, [3, 1]],
    ['to zero', [2, 1], 1, 0, 5, [2, 0]],
    ['past what is left', [2, 1], 0, 9, 5, [4, 1]],
    ['when everything is assigned', [3, 2], 0, 4, 5, [3, 2]],
    ['to less than zero', [2, 1], 0, -3, 5, [0, 1]],
    ['to a fraction', [0, 0], 1, 2.9, 5, [0, 2]],
    ['to not a number', [2, 1], 1, Number.NaN, 5, [2, 0]],
    ['to infinity', [1, 1], 0, Number.POSITIVE_INFINITY, 5, [4, 1]],
    ['with no power', [0, 0], 0, 1, 0, [0, 0]],
  ])('%s', (_what, amounts, index, wanted, power, expected) => {
    expect(setAmount(amounts, index, wanted, power)).toEqual(expected)
  })

  it('ignores a row that is not there', () => {
    expect(setAmount([1, 1], 5, 3, 9)).toEqual([1, 1])
    expect(setAmount([1, 1], -1, 3, 9)).toEqual([1, 1])
  })

  it('never changes the amounts it is given', () => {
    const amounts = [1, 2]
    setAmount(amounts, 0, 3, 9)
    expect(amounts).toEqual([1, 2])
  })

  it.each(POWERS)('never lets any sequence of changes pass a power of %i', (power) => {
    let amounts = [0, 0, 0]
    for (let step = 0; step < 40; step += 1) {
      amounts = setAmount(amounts, (step * 7) % 3, ((step * 13) % 17) - 4, power)
      expect(totalOf(amounts)).toBeLessThanOrEqual(power)
      expect(amounts.every((amount) => Number.isInteger(amount) && amount >= 0)).toBe(true)
    }
  })

  it('tells a row the most it may take', () => {
    expect(clampedAmount([2, 1], 0, 99, 5)).toBe(4)
    expect(clampedAmount([2, 1], 1, 99, 5)).toBe(3)
  })
})

describe('judging a split', () => {
  it.each([
    [5, [2, 3], true, 'Assigned 5 of 5. Ready to confirm.'],
    [5, [0, 5], true, 'Assigned 5 of 5. Ready to confirm.'],
    [5, [2, 2], false, 'Assigned 4 of 5. Assign 1 more to confirm.'],
    [5, [0, 0, 0], false, 'Assigned 0 of 5. Assign 5 more to confirm.'],
    [5, [3, 4], false, 'Assigned 7 of 5. Take away 2 to confirm.'],
    [1, [1], true, 'Assigned 1 of 1. Ready to confirm.'],
    [0, [0, 0], true, 'There is no damage to assign.'],
    [0, [], true, 'There is no damage to assign.'],
    [3, [], false, 'There is nothing to assign this damage to.'],
    [4, [2.5, 1.5], false, 'Assigned 4 of 4. Every share must be a whole number, 0 or more.'],
    [4, [6, -2], false, 'Assigned 4 of 4. Every share must be a whole number, 0 or more.'],
  ])('%i damage as %j is %s: %s', (power, amounts, legal, status) => {
    expect(judge(power, amounts)).toEqual({ legal, status })
  })

  it.each(POWERS)('of %i damage is legal only when the total matches, whatever the lineup', (power) => {
    const wrong: string[] = []
    for (const line of LINEUPS) {
      for (const delta of [-2, -1, 0, 1, 2]) {
        const split = defaultSplit(power, line)
        const last = split.length - 1
        split[last] = (split[last] ?? 0) + delta
        if (judge(power, split).legal !== (delta === 0)) wrong.push(`${show(line)} ${delta}`)
      }
    }
    expect(wrong).toEqual([])
  })
})

describe('lethal damage', () => {
  it.each([
    ['toughness less damage', blocker(1, 4, 1), false, 3],
    ['nothing when damage already passes the toughness', blocker(1, 2, 5), false, 0],
    ['one from deathtouch where more is needed', blocker(1, 4, 1), true, 1],
    ['nothing from deathtouch when lethal damage is already marked', blocker(1, 2, 2), true, 0],
    ['one from deathtouch against a toughness of one', blocker(1, 1), true, 1],
  ])('is %s', (_what, recipient, deathtouch, lethal) => {
    expect(lethalDamage(recipient, deathtouch)).toBe(lethal)
  })

  it('is unknown without a toughness, deathtouch or not', () => {
    expect(lethalDamage(blocker(1, null))).toBeNull()
    expect(lethalDamage(blocker(1, null), true)).toBeNull()
  })

  it('is the engine’s figure when the decision carries one, whatever the numbers on the creature say', () => {
    const told = { ...blocker(1, 5, 0), lethal: 1 }
    expect(lethalDamage(told)).toBe(1)
    expect(lethalDamage({ ...told, lethal: 0 }, true)).toBe(0)
    expect(lethalDamage({ ...blocker(1, null), lethal: 2 })).toBe(2)
  })

  it('names deathtouch where it makes the difference, and only there', () => {
    expect(factsOf(blocker(1, 4, 1), true)).toBe('Toughness 4, 1 damage marked, lethal: 1 (deathtouch)')
    expect(factsOf(blocker(1, 1), true)).toBe('Toughness 1, lethal: 1')
    expect(factsOf(blocker(1, 4, 1), false)).toBe('Toughness 4, 1 damage marked, lethal: 3')
    expect(factsOf(blocker(1, 2, 2), true)).toBe('Toughness 2, 2 damage marked, lethal damage already marked')
  })

  it('says how far a share is from lethal with deathtouch counted', () => {
    expect(lethalNote(blocker(1, 5), 0, true)).toBe('1 short of lethal')
    expect(lethalNote(blocker(1, 5), 1, true)).toBe('Lethal')
    expect(lethalNote(blocker(1, 5), 1, false)).toBe('4 short of lethal')
  })

  it('knows deathtouch from the keyword names of the attacker', () => {
    expect(hasDeathtouch(['Flying', 'Deathtouch'])).toBe(true)
    expect(hasDeathtouch(['First strike'])).toBe(false)
    expect(hasDeathtouch([])).toBe(false)
    expect(hasDeathtouch(undefined)).toBe(false)
  })
})

describe('the split of an attacker with trample', () => {
  const trampling = { trample: true }
  const line = (...lethals: number[]): DamageRecipient[] =>
    lethals.map((lethal, index) => ({ ...blocker(index + 1, 9), lethal }))
  const rule = (recipients: DamageRecipient[], deathtouch = false): Trample => ({
    to: 'Bo',
    recipients,
    deathtouch,
  })

  it.each([
    [7, [2, 3], [2, 3]],
    [4, [2, 3], [2, 2]],
    [5, [2, 3], [2, 3]],
    [3, [0], [0]],
    [0, [2, 2], [0, 0]],
    [10, [1, 1, 1], [1, 1, 1]],
  ])(
    'starts with lethal damage to each blocker and keeps the rest back: %i over %j is %j',
    (power, lethals, split) => {
      expect(defaultSplit(power, line(...lethals), trampling)).toEqual(split)
    },
  )

  it.each([
    [7, [2, 3], 2],
    [4, [2, 3], 0],
    [3, [0], 3],
    [10, [1, 1, 1], 7],
  ])(
    'leaves the player or planeswalker attacked what lethal damage does not use: %i over %j leaves %i',
    (power, lethals, rest) => {
      expect(restOf(power, defaultSplit(power, line(...lethals), trampling))).toBe(rest)
    },
  )

  it('works lethal damage out from the toughness and deathtouch when the decision does not say', () => {
    const plain = [blocker(1, 5), blocker(2, 5)]
    expect(defaultSplit(8, plain, trampling)).toEqual([5, 3])
    expect(defaultSplit(8, plain, { trample: true, deathtouch: true })).toEqual([1, 1])
    expect(restOf(8, defaultSplit(8, plain, { trample: true, deathtouch: true }))).toBe(6)
  })

  it('judges a split legal when it falls short only past blockers that all have lethal damage', () => {
    const blockers = line(2, 3)
    const trample = rule(blockers)
    expect(judge(7, [2, 3], trample)).toEqual({
      legal: true,
      status: 'Assigned 5 of 7. The other 2 goes to Bo. Ready to confirm.',
    })
    expect(judge(7, [3, 4], trample)).toEqual({ legal: true, status: 'Assigned 7 of 7. Ready to confirm.' })
    const short = judge(7, [1, 3], trample)
    expect(short.legal).toBe(false)
    expect(short.status).toBe(
      'Assigned 4 of 7. Give every blocker its lethal damage to send the rest to Bo, or assign 3 more.',
    )
    expect(judge(7, [4, 4], trample)).toEqual({
      legal: false,
      status: 'Assigned 8 of 7. Take away 1 to confirm.',
    })
  })

  it('judges the same split by the rules of the attacker: short is illegal without trample', () => {
    expect(judge(7, [2, 3])).toEqual({ legal: false, status: 'Assigned 5 of 7. Assign 2 more to confirm.' })
  })

  it('counts deathtouch towards lethal for the blockers the decision does not give a figure for', () => {
    const plain = [blocker(1, 5), blocker(2, 5)]
    expect(judge(4, [1, 1], rule(plain, true)).legal).toBe(true)
    expect(judge(4, [1, 1], rule(plain, false)).legal).toBe(false)
    expect(lethalEverywhere(plain, [1, 0], true)).toBe(false)
    expect(lethalEverywhere(plain, [1, 1], true)).toBe(true)
  })

  it('sends everything to the player or planeswalker when there is no blocker left to give to', () => {
    expect(judge(4, [], rule([]))).toEqual({ legal: true, status: 'All 4 goes to Bo.' })
    expect(judge(0, [], rule([]))).toEqual({ legal: true, status: 'There is no damage to assign.' })
  })

  it('is accepted by the engine for every power and lineup: the default split is legal, and any short of lethal is not', () => {
    const wrong: string[] = []
    for (const power of POWERS) {
      for (const lethals of [[1], [2], [0, 3], [2, 2], [1, 1, 1], [3, 0, 2]]) {
        const blockers = line(...lethals)
        const split = defaultSplit(power, blockers, trampling)
        if (!judge(power, split, rule(blockers)).legal) wrong.push(`${power} over ${lethals.join()}`)
        const needs = lethals.reduce((sum, lethal) => sum + lethal, 0)
        if (power > needs) {
          // Taking one point from a blocker that needs it, leaving it for the player, is never legal.
          const index = lethals.findIndex((lethal) => lethal > 0)
          if (index >= 0) {
            const short = split.map((amount, at) => (at === index ? amount - 1 : amount))
            if (judge(power, short, rule(blockers)).legal) wrong.push(`short ${power} over ${lethals.join()}`)
          }
        }
      }
    }
    expect(wrong).toEqual([])
  })
})

describe('the answer', () => {
  const line = [blocker(7, 2), blocker(8, 2), blocker(9, 2)]

  it.each([
    [
      [2, 0, 3],
      [
        [7, 2],
        [9, 3],
      ],
    ],
    [[0, 0, 5], [[9, 5]]],
    [
      [1, 2, 2],
      [
        [7, 1],
        [8, 2],
        [9, 2],
      ],
    ],
    [[0, 0, 0], []],
  ])(
    'for amounts %j names only the blockers that get damage, in the offered order: %j',
    (amounts, shares) => {
      expect(toAnswer(line, amounts)).toEqual({ AssignDamage: shares })
    },
  )
})

describe('names', () => {
  it.each([
    [['Elf'], ['Elf']],
    [
      ['Elf', 'Bear'],
      ['Elf', 'Bear'],
    ],
    [
      ['Elf', 'Elf'],
      ['Elf (1 of 2)', 'Elf (2 of 2)'],
    ],
    [
      ['Elf', 'Bear', 'Elf', 'Elf'],
      ['Elf (1 of 3)', 'Bear', 'Elf (2 of 3)', 'Elf (3 of 3)'],
    ],
    [[], []],
  ])('%j are shown as %j', (names, shown) => {
    expect(labelled(names)).toEqual(shown)
  })
})
