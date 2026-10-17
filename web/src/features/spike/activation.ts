import type { GameObject, ObjectId, TargetDecision } from '@/game/types'
import { battlefieldIds } from './simulator'

export type Activation =
  | { kind: 'begin'; decision: TargetDecision }
  | { kind: 'resolve'; source: ObjectId; target: ObjectId }
  | { kind: 'cancel' }
  | { kind: 'ignore' }

/** Decides what a click on `id` means given the current pending decision (stands in for the server). */
export function interpretActivation(
  objects: Readonly<Record<ObjectId, GameObject>>,
  decision: TargetDecision | null,
  id: ObjectId,
): Activation {
  if (decision) {
    return decision.legal.has(id)
      ? { kind: 'resolve', source: decision.source, target: id }
      : { kind: 'cancel' }
  }
  const object = objects[id]
  if (!object?.playable) return { kind: 'ignore' }
  const others = Object.values(objects).filter((candidate) => candidate.id !== id)
  return { kind: 'begin', decision: { source: id, legal: battlefieldIds(others) } }
}
