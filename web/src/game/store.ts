import { create } from 'zustand'
import type { GameObject, ObjectId, ObjectPatch, TargetDecision } from './types'
import { buildZoneIndex, movesZone, type ZoneIndex } from './zones'

export interface GameState {
  objects: Readonly<Record<ObjectId, GameObject>>
  zones: ZoneIndex
  decision: TargetDecision | null
  hovered: ObjectId | null
  load: (objects: readonly GameObject[]) => void
  applyPatches: (patches: readonly ObjectPatch[]) => void
  setDecision: (decision: TargetDecision | null) => void
  setHovered: (id: ObjectId | null) => void
}

/** Applies patches, keeping the identity of every object and list that did not change. */
export function reducePatches(
  objects: GameState['objects'],
  zones: ZoneIndex,
  patches: readonly ObjectPatch[],
): Pick<GameState, 'objects' | 'zones'> {
  const next: Record<ObjectId, GameObject> = { ...objects }
  let zonesDirty = false
  for (const { id, changes } of patches) {
    const before = next[id]
    if (!before) continue
    const after = { ...before, ...changes }
    if (movesZone(before, after)) zonesDirty = true
    next[id] = after
  }
  return { objects: next, zones: zonesDirty ? buildZoneIndex(Object.values(next)) : zones }
}

export const useGameStore = create<GameState>((set) => ({
  objects: {},
  zones: {},
  decision: null,
  hovered: null,
  load: (list) =>
    set({
      objects: Object.fromEntries(list.map((object) => [object.id, object])),
      zones: buildZoneIndex(list),
      decision: null,
    }),
  applyPatches: (patches) => set((state) => reducePatches(state.objects, state.zones, patches)),
  setDecision: (decision) => set({ decision }),
  setHovered: (hovered) => set({ hovered }),
}))

export const useObject = (id: ObjectId): GameObject | undefined => useGameStore((state) => state.objects[id])
