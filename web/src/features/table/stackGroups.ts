/**
 * Folding a crowded stack: items next to each other that are the same ability of the same source with the same text
 * (a board of triggers), or copies of the same spell, make one group with a count. Pure.
 */
import type { ObjectId, ObjectView } from '@/protocol/generated'

/** Items of the stack that show as one chip or row, top first. */
export interface StackGroup {
  /** Stable while the bottom member stays: the top ones resolve first. */
  key: string
  /** The members, top first; the first is the one that resolves first and stands for the group. */
  ids: readonly ObjectId[]
}

/** What makes two items one: abilities of a source with the same text, or copies of a spell, of one controller. */
function foldKey(object: ObjectView | undefined): string | null {
  if (object === undefined) return null
  const shared = `${object.controller}|${object.name}|${object.rules_text ?? ''}`
  if (object.kind === 'Ability') return `ability|${object.source ?? ''}|${shared}`
  if (object.copy !== undefined) return `copy|${object.kind ?? 'Card'}|${shared}`
  return null
}

/** The stack, top first, with each run of items that fold together as one group; any other item is a group of one. */
export function groupStack(
  ids: readonly ObjectId[],
  objectsById: Readonly<Record<ObjectId, ObjectView | undefined>>,
): StackGroup[] {
  const groups: { fold: string | null; ids: ObjectId[] }[] = []
  for (const id of ids) {
    const fold = foldKey(objectsById[id])
    const last = groups.at(-1)
    if (last !== undefined && fold !== null && last.fold === fold) last.ids.push(id)
    else groups.push({ fold, ids: [id] })
  }
  return groups.map((group) => ({ key: String(group.ids.at(-1)), ids: group.ids }))
}

/**
 * How many chips of a row fit: `rights` are the right edges of the chips from the row's start, in order, and `width`
 * is the row's own width. The first chip always counts, so the top of the stack is never folded away.
 */
export function chipsThatFit(rights: readonly number[], width: number): number {
  const fit = rights.findIndex((right) => right > width + 0.5)
  return fit === -1 ? rights.length : Math.max(1, fit)
}
