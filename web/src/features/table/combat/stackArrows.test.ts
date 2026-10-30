// Goal: the arrows of the stack run from the chip of the item on show (the one hovered or focused, else the top) to each target still legal: a card, a seat, or another item's chip; at most 6, none for a gone target, in the viewer's colour only for the viewer's own item.
import { describe, expect, it } from 'vitest'
import type { StackEntry, StackTarget } from '../stackTargets'
import { anchorKey } from './plan'
import { planStackArrows, shownEntry } from './stackArrows'

const card = (id: number, over: Partial<StackTarget> = {}): StackTarget => ({
  key: `object:${id}:0`,
  target: { Object: { id, counter: 0 } },
  gone: false,
  onStack: false,
  seat: 1,
  ...over,
})
const seat = (n: number): StackTarget => ({
  key: `player:${n}`,
  target: { Player: n },
  gone: false,
  onStack: false,
})
const item = (id: number, controller: number, ...targets: StackTarget[]): StackEntry => ({
  id,
  controller,
  targets,
})

const top = item(10, 0, card(1), seat(1))
const second = item(11, 1, card(2))
const keys = (lines: ReturnType<typeof planStackArrows>) =>
  lines.map((l) => [anchorKey(l.from), anchorKey(l.to)])

describe('who points where', () => {
  it('draws the top item to its cards and seats, from its chip', () => {
    const lines = planStackArrows([top, second], null, 0)
    expect(keys(lines)).toEqual([
      ['stack:10', 'card:1'],
      ['stack:10', 'seat:1'],
    ])
    expect(lines.every((line) => line.kind === 'target' && !line.pending)).toBe(true)
  })

  it('lets a card that is not drawn stand in as its controller through the anchor seat', () => {
    expect(planStackArrows([top], null, 0)[0]?.to).toEqual({ kind: 'card', id: 1, seat: 1 })
  })

  it('draws the item that is hovered or focused instead of the top', () => {
    expect(keys(planStackArrows([top, second], 11, 0))).toEqual([['stack:11', 'card:2']])
    expect(shownEntry([top, second], 99)).toBe(top)
    expect(shownEntry([], 11)).toBeUndefined()
  })

  it('points at the chip of a target that is itself on the stack', () => {
    const counter = item(12, 0, card(10, { onStack: true }))
    expect(keys(planStackArrows([counter, top], null, 0))).toEqual([['stack:12', 'stack:10']])
  })

  it('draws nothing for a target that is gone, an item that aims at nothing or an empty stack', () => {
    expect(keys(planStackArrows([item(10, 0, card(1, { gone: true }), seat(1))], null, 0))).toEqual([
      ['stack:10', 'seat:1'],
    ])
    expect(planStackArrows([item(10, 0)], null, 0)).toEqual([])
    expect(planStackArrows([], null, 0)).toEqual([])
  })
})

describe('the cap', () => {
  it.each([
    [3, 3],
    [6, 6],
    [7, 6],
    [20, 6],
  ])('draws %i targets as %i arrows', (count, drawn) => {
    const many = item(10, 0, ...Array.from({ length: count }, (_, id) => card(id + 1)))
    expect(planStackArrows([many], null, 0)).toHaveLength(drawn)
  })

  it('gives each arrow a key of its own that stays while the item and the target stay', () => {
    const lines = planStackArrows([top], null, 0)
    expect(new Set(lines.map((l) => l.key)).size).toBe(lines.length)
    expect(planStackArrows([top], null, 1).map((l) => l.key)).toEqual(lines.map((l) => l.key))
  })
})

describe('whose arrows', () => {
  it('marks the arrows of the viewer own item and not another seat', () => {
    expect(planStackArrows([top], null, 0).every((line) => line.mine === true)).toBe(true)
    expect(planStackArrows([top], null, 1).every((line) => line.mine === false)).toBe(true)
    expect(planStackArrows([top], null, null).every((line) => line.mine === false)).toBe(true)
  })
})
