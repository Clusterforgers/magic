// Goal: a cost becomes one slot a mana (a generic part a slot each), the payment fills the slot that asks for its kind before one that takes any, a source that costs mana adds slots of its own that its mana also fills, life fills the symbol it pays, and generic-only payments never fill a coloured slot.
import { describe, expect, it } from 'vitest'
import type { ManaCost, ManaOption, ManaType } from '@/protocol/generated'
import type { Spend } from './payment'
import { paidCount, paySlots } from './paySlots'

const W: ManaCost[number] = { Colored: 'White' }
const U: ManaCost[number] = { Colored: 'Blue' }
const BRAGO: ManaCost = [{ Generic: 2 }, W, U]

const tap = (object: number, produces: ManaType, extra: Partial<ManaOption> = {}): Spend => ({
  kind: 'tap',
  option: { source: { object, ability: 0 }, produces, ...extra },
})
const SIGNET = tap(5, 'White', { also: ['Blue'], cost: [{ Generic: 1 }] })

/** Each slot as what it accepts and the spend that fills it: `any<-0`, `White<-`, `x:any<-2`. */
const shape = (cost: ManaCost, spends: readonly Spend[], life: readonly number[] = []): string[] =>
  paySlots(cost, spends, life).map((slot) => {
    const wants = slot.accepts === null ? 'any' : slot.accepts.join('/')
    const by = slot.fill === null ? '' : slot.fill.life ? 'life' : String(slot.fill.spend)
    return `${slot.extraOf === undefined ? '' : 'x:'}${wants}<-${by}`
  })

describe('the slots of a payment', () => {
  it.each<[string, ManaCost, Spend[], string[]]>([
    ['nothing paid', BRAGO, [], ['any<-', 'any<-', 'White<-', 'Blue<-']],
    ['a Plains pays the white', BRAGO, [tap(1, 'White')], ['any<-', 'any<-', 'White<-0', 'Blue<-']],
    [
      'a second white goes to a generic slot',
      BRAGO,
      [tap(1, 'White'), tap(2, 'White')],
      ['any<-1', 'any<-', 'White<-0', 'Blue<-'],
    ],
    [
      'a land of either colour pays the colour it was tapped for',
      BRAGO,
      [tap(3, 'Blue'), tap(4, 'White')],
      ['any<-', 'any<-', 'White<-1', 'Blue<-0'],
    ],
    [
      'a Signet pays two and owes one of its own, which the next mana fills',
      BRAGO,
      [tap(1, 'White'), SIGNET, tap(2, 'Blue')],
      ['any<-1', 'any<-2', 'White<-0', 'Blue<-1', 'x:any<-'],
    ],
    [
      'the whole cost and the Signet',
      BRAGO,
      [tap(1, 'White'), SIGNET, tap(2, 'Blue'), tap(3, 'White')],
      ['any<-1', 'any<-2', 'White<-0', 'Blue<-1', 'x:any<-3'],
    ],
    [
      'floating mana and improvise',
      [{ Generic: 1 }, W],
      [
        { kind: 'improvise', artifact: 9 },
        { kind: 'pool', mana: 'White' },
      ],
      ['any<-0', 'White<-1'],
    ],
    ['improvise never pays a colour', [W], [{ kind: 'improvise', artifact: 9 }], ['White<-']],
    [
      'a hybrid symbol takes either colour',
      [{ Hybrid: ['White', 'Blue'] }],
      [tap(1, 'Blue')],
      ['White/Blue<-0'],
    ],
    [
      'the narrower slot is filled first',
      [{ Hybrid: ['White', 'Blue'] }, W],
      [tap(1, 'White'), tap(2, 'Blue')],
      ['White/Blue<-1', 'White<-0'],
    ],
    ['a free cost has no slot', [], [], []],
  ])('%s', (_what, cost, spends, slots) => {
    expect(shape(cost, spends)).toEqual(slots)
  })

  it('fills a Phyrexian symbol with the life that pays it', () => {
    expect(shape([{ Phyrexian: 'White' }, { Generic: 1 }], [], [0])).toEqual(['White<-life', 'any<-'])
  })

  it.each<[Spend[], number]>([
    [[], 0],
    [[tap(1, 'White')], 1],
    [[tap(1, 'White'), SIGNET], 3],
  ])('counts what is filled', (spends, paid) => {
    expect(paidCount(paySlots(BRAGO, spends))).toBe(paid)
  })

  it('names the source of an extra slot', () => {
    const [extra] = paySlots(BRAGO, [SIGNET]).filter((slot) => slot.extraOf !== undefined)
    expect(extra?.extraOf).toBe(5)
  })
})
