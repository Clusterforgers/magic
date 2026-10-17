import type { GameObject, ObjectId, PlayerId, Zone } from './types'

export type ZoneIndex = Readonly<Record<string, readonly ObjectId[]>>

export const zoneKey = (player: PlayerId, zone: Zone): string => `${player}:${zone}`

/** Builds the per-zone ordered id lists from a set of objects. */
export function buildZoneIndex(objects: Iterable<GameObject>): ZoneIndex {
  const index: Record<string, ObjectId[]> = {}
  for (const object of objects) {
    const key = zoneKey(object.controller, object.zone)
    ;(index[key] ??= []).push(object.id)
  }
  return index
}

/** Whether a change to `before` can alter which zone list it belongs to. */
export function movesZone(before: GameObject, after: GameObject): boolean {
  return before.zone !== after.zone || before.controller !== after.controller
}
