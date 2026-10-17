// Goal: click semantics for the guided-play flow: begin on playable, resolve on legal, cancel otherwise.
import { describe, expect, it } from 'vitest'
import { interpretActivation } from './activation'
import type { GameObject } from '@/game/types'

const make = (id: number, over: Partial<GameObject>): GameObject => ({
  id,
  name: 'x',
  controller: 0,
  zone: 'hand',
  tapped: false,
  attacking: false,
  power: 1,
  toughness: 1,
  damage: 0,
  playable: false,
  ...over,
})
const objects = {
  1: make(1, { playable: true }),
  2: make(2, { zone: 'battlefield' }),
  3: make(3, { zone: 'hand' }),
}

describe('interpretActivation', () => {
  it('begins targeting from a playable card with battlefield targets only', () => {
    const result = interpretActivation(objects, null, 1)
    expect(result).toMatchObject({ kind: 'begin' })
    if (result.kind === 'begin') expect([...result.decision.legal]).toEqual([2])
  })

  it('ignores clicks on non-playable cards when nothing is pending', () => {
    expect(interpretActivation(objects, null, 3)).toEqual({ kind: 'ignore' })
  })

  it('resolves on a legal target and cancels on an illegal one', () => {
    const decision = { source: 1, legal: new Set([2]) }
    expect(interpretActivation(objects, decision, 2)).toEqual({ kind: 'resolve', source: 1, target: 2 })
    expect(interpretActivation(objects, decision, 3)).toEqual({ kind: 'cancel' })
  })
})
