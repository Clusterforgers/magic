// Goal: the targets of a stack item read as a short chip line ("Hill Giant, Bo +2 more"), a name plus targets ("Lightning Bolt → Hill Giant") and a screen reader sentence, naming only targets that are still legal and saying so when none is left.
import { describe, expect, it } from 'vitest'
import { arrowsLeftOut, chipLine, targetsAria, targetsLine } from './stackText'
import type { TargetNamer } from './stackText'
import type { StackEntry, StackTarget } from './stackTargets'

const NAMES = [
  'Hill Giant',
  'Grizzly Bears',
  'Serra Angel',
  'Wind Drake',
  'Forest',
  'Island',
  'Swamp',
  'Plains',
]
const n: TargetNamer = { seat: (seat) => ['Ana', 'Bo'][seat] ?? 'Seat', card: (id) => NAMES[id] ?? 'a card' }

const object = (id: number, gone = false): StackTarget => ({
  key: `object:${id}:0`,
  target: { Object: { id, counter: 0 } },
  gone,
  onStack: false,
})
const player = (seat: number, gone = false): StackTarget => ({
  key: `player:${seat}`,
  target: { Player: seat },
  gone,
  onStack: false,
})
const entry = (...targets: StackTarget[]): StackEntry => ({ id: 9, controller: 0, targets })

describe('the chip line', () => {
  it.each([
    ['nothing for an item that aims at nothing', entry(), ''],
    ['one card', entry(object(0)), 'Hill Giant'],
    ['a card and a player', entry(object(0), player(1)), 'Hill Giant, Bo'],
    [
      'three names without "more"',
      entry(object(0), object(1), object(2)),
      'Hill Giant, Grizzly Bears, Serra Angel',
    ],
    [
      'the first three and the rest counted',
      entry(...[0, 1, 2, 3, 4, 5, 6].map((id) => object(id))),
      'Hill Giant, Grizzly Bears, Serra Angel +4 more',
    ],
    ['only the targets still legal', entry(object(0, true), object(1)), 'Grizzly Bears'],
    ['the marker when every target is gone', entry(object(0, true), player(1, true)), 'no legal target'],
  ])('is %s', (_, aimed, line) => {
    expect(targetsLine(aimed, n)).toBe(line)
  })

  it('puts the name before an arrow and the targets', () => {
    expect(chipLine('Lightning Bolt', entry(object(0)), n)).toBe('Lightning Bolt → Hill Giant')
    expect(chipLine('Giant Growth', entry(), n)).toBe('Giant Growth')
  })
})

describe('the screen reader text', () => {
  it.each([
    ['nothing without targets', entry(), ''],
    ['one target', entry(object(0)), ', targeting Hill Giant'],
    ['two targets joined with "and"', entry(object(0), player(1)), ', targeting Hill Giant and Bo'],
    [
      'three targets as a list',
      entry(object(0), object(1), player(0)),
      ', targeting Hill Giant, Grizzly Bears and Ana',
    ],
    [
      'the first three and the count of the rest',
      entry(...[0, 1, 2, 3, 4].map((id) => object(id))),
      ', targeting Hill Giant, Grizzly Bears, Serra Angel and 2 more',
    ],
    ['how many are gone', entry(object(0), object(1, true)), ', targeting Hill Giant, 1 no longer legal'],
    ['that nothing is left', entry(object(0, true)), ', no legal target left'],
  ])('says %s', (_, aimed, text) => {
    expect(targetsAria(aimed, n)).toBe(text)
  })
})

describe('the arrows left out', () => {
  it.each([
    [0, 0],
    [6, 0],
    [7, 1],
    [9, 3],
  ])('counts %i targets as %i arrows left out', (count, left) => {
    expect(arrowsLeftOut(entry(...Array.from({ length: count }, (_, id) => object(id))))).toBe(left)
  })
})
