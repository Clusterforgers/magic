/**
 * What a pointed stack item lights on the table (`docs/ui-design.md`, Hover and focus): a panel item, a strip chip or
 * the asking chip, hovered, focused or pressed on touch, opens its preview (`usePreviewTarget`), and its source and its
 * targets that stand on the table light as the asking card does. Pure: `AskMarks` draws the result by object id.
 */
import type { ObjectId, ObjectView } from '@/protocol/generated'
import type { StackEntry } from './stackTargets'

/**
 * The objects the stack item `id` lights: its source (the permanent of an ability, unless it is the item itself) and
 * its targets that are objects and still legal; none when `id` is not on the stack.
 */
export function stackLights(
  id: ObjectId | null,
  objectsById: Readonly<Record<ObjectId, ObjectView | undefined>>,
  entries: readonly StackEntry[],
): ObjectId[] {
  if (id === null) return []
  const object = objectsById[id]
  if (object === undefined || object.zone !== 'Stack') return []
  const lit: ObjectId[] = []
  if (object.source !== undefined && object.source !== id) lit.push(object.source)
  const entry = entries.find((one) => one.id === id)
  for (const target of entry?.targets ?? []) {
    if (target.gone || !('Object' in target.target)) continue
    const at = target.target.Object.id
    if (!lit.includes(at)) lit.push(at)
  }
  return lit
}
