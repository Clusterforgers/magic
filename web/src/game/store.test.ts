// Goal: patch application keeps unchanged objects and zone lists referentially stable, so unrelated cards never re-render.
import { describe, expect, it } from 'vitest'
import { createBoard } from '@/features/spike/simulator'
import { reducePatches } from './store'
import { buildZoneIndex, zoneKey } from './zones'
import type { ObjectPatch } from './types'

const board = createBoard({ players: 2, permanentsPerPlayer: 5, handSize: 2, seed: 7 })
const objects = Object.fromEntries(board.map((object) => [object.id, object]))
const zones = buildZoneIndex(board)

describe('reducePatches', () => {
  it.each<[string, ObjectPatch[]]>([
    ['tap', [{ id: 1, changes: { tapped: true } }]],
    ['damage', [{ id: 2, changes: { damage: 2 } }]],
    ['attack', [{ id: 3, changes: { attacking: true } }]],
  ])('%s change leaves other objects and zone lists untouched', (_, patches) => {
    const result = reducePatches(objects, zones, patches)
    const changed = new Set(patches.map((patch) => patch.id))
    for (const object of board) {
      if (!changed.has(object.id)) expect(result.objects[object.id]).toBe(object)
    }
    expect(result.zones).toBe(zones)
  })

  it('rebuilds zone lists only when an object moves zone', () => {
    const hand = board.find((object) => object.zone === 'hand')
    expect(hand).toBeDefined()
    if (!hand) return
    const result = reducePatches(objects, zones, [{ id: hand.id, changes: { zone: 'battlefield' } }])
    expect(result.zones).not.toBe(zones)
    expect(result.zones[zoneKey(hand.controller, 'battlefield')]).toContain(hand.id)
    expect(result.zones[zoneKey(hand.controller, 'hand')]).not.toContain(hand.id)
  })

  it('ignores patches for unknown ids', () => {
    const result = reducePatches(objects, zones, [{ id: 9999, changes: { tapped: true } }])
    expect(result.objects[9999]).toBeUndefined()
  })
})
