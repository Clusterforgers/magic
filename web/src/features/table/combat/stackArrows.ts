/**
 * The arrows of the stack, as data: from the chip of one stack item to each of its targets that is still legal. The
 * item is the one the viewer hovers or focuses, else the top of the stack. Pure; `CombatOverlay` draws them.
 */
import type { ObjectId, PlayerId } from '@/protocol/generated'
import { live, MAX_ARROWS } from '../stackTargets'
import type { StackEntry, StackTarget } from '../stackTargets'
import type { StackGroup } from '../stackGroups'
import type { Anchor, LineSpec } from './plan'

/** The item whose arrows are drawn: `shown` when it is on the stack, else the top of the stack. */
export function shownEntry(entries: readonly StackEntry[], shown: ObjectId | null): StackEntry | undefined {
  return entries.find((entry) => entry.id === shown) ?? entries[0]
}

function anchorOf({ target, onStack, seat }: StackTarget): Anchor {
  if ('Player' in target) return { kind: 'seat', seat: target.Player }
  if (onStack) return { kind: 'stack', id: target.Object.id }
  return seat === undefined
    ? { kind: 'card', id: target.Object.id }
    : { kind: 'card', id: target.Object.id, seat }
}

/** The items whose arrows are drawn: the whole run of the folded chip that holds the shown item, else the shown one. */
export function shownEntries(
  entries: readonly StackEntry[],
  shown: ObjectId | null,
  groups: readonly StackGroup[] = [],
): StackEntry[] {
  const group = groups.find((candidate) => shown !== null && candidate.ids.includes(shown))
  if (group === undefined) {
    const entry = shownEntry(entries, shown)
    return entry === undefined ? [] : [entry]
  }
  return entries.filter((entry) => group.ids.includes(entry.id))
}

/** The arrows for the items shown: at most `MAX_ARROWS` each, none for a target that is gone, none for an item that aims at nothing. */
export function planStackArrows(
  entries: readonly StackEntry[],
  shown: ObjectId | null,
  viewer: PlayerId | null,
  groups: readonly StackGroup[] = [],
): LineSpec[] {
  return shownEntries(entries, shown, groups).flatMap((entry) =>
    live(entry)
      .slice(0, MAX_ARROWS)
      .map((target): LineSpec => ({
        key: `target:${entry.id}:${target.key}`,
        kind: 'target',
        pending: false,
        from: { kind: 'stack', id: entry.id },
        to: anchorOf(target),
        mine: entry.controller === viewer,
      })),
  )
}
