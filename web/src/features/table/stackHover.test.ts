//! Goal: a pointed stack item lights its source and its targets that are objects and still legal, each once, and nothing for an object that is not on the stack or for no item at all.
import { describe, expect, it } from 'vitest'
import type { ObjectView } from '@/protocol/generated'
import { stackLights } from './stackHover'
import type { StackEntry, StackTarget } from './stackTargets'

const object = (id: number, zone: string, source?: number): ObjectView =>
  ({ id, zone, ...(source === undefined ? {} : { source }) }) as unknown as ObjectView

const target = (id: number, gone = false): StackTarget => ({
  key: `o${id}`,
  target: { Object: { id, counter: 0 } },
  gone,
  onStack: false,
})
const player = (seat: number): StackTarget => ({
  key: `p${seat}`,
  target: { Player: seat },
  gone: false,
  onStack: false,
})

const objects = {
  1: object(1, 'Stack', 5),
  2: object(2, 'Stack'),
  3: object(3, 'Battlefield'),
  5: object(5, 'Battlefield'),
}
const entries: StackEntry[] = [
  { id: 1, controller: 0, targets: [target(3), target(4, true), player(1), target(3)] },
  { id: 2, controller: 1, targets: [target(1)] },
]

describe('stackLights', () => {
  it.each([
    ['an ability: its source, then its legal object targets once', 1, [5, 3]],
    ['a spell targeting another stack item', 2, [1]],
    ['a permanent, not on the stack', 3, []],
    ['nothing pointed', null, []],
    ['an id the view does not hold', 9, []],
  ] as const)('lights for %s', (_what, id, lit) => {
    expect(stackLights(id, objects, entries)).toEqual(lit)
  })
})
