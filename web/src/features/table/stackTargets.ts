/**
 * What the spells and abilities on the stack aim at, as data: each target once, in order, with whether it is still
 * there. Pure; the words are `stackText.ts`, the arrows `combat/stackArrows.ts`.
 */
import type { LogEntry } from '@/play/store'
import type { Event, ObjectId, ObjectView, PlayerId, PlayerView, Target } from '@/protocol/generated'
import { distinctTargets, targetKey } from './prompt/targetText'

/** Names shown on a chip before "+N more". */
export const MAX_NAMED = 3

/** Arrows drawn from one stack item; the rest of its targets are "+N more". */
export const MAX_ARROWS = 6

/** One target of a stack item. */
export interface StackTarget {
  key: string
  target: Target
  /** No longer a legal target: the server says so (`targets_legal`), or else the player lost or the object moved since. */
  gone: boolean
  /** An object that is itself on the stack (a Counterspell's target). */
  onStack: boolean
  /** Who controls the object, so that a card that is not drawn stands in as its controller's seat. */
  seat?: PlayerId
}

/** A spell or ability on the stack and its targets. */
export interface StackEntry {
  id: ObjectId
  controller: PlayerId
  targets: readonly StackTarget[]
}

/** The events that end an object's time where a target was named: it moved, left the game or ended its resolution. */
function leaver(event: Event): ObjectId | null {
  if (typeof event === 'string') return null
  if ('ZoneChanged' in event) return event.ZoneChanged.object
  if ('ObjectLeftGame' in event) return event.ObjectLeftGame.object
  if ('SpellResolved' in event) return event.SpellResolved.object
  if ('ResolutionEnded' in event) return event.ResolutionEnded.object
  return null
}

/** The event that put `item` on the stack. */
function putsOnStack(event: Event, item: ObjectId): boolean {
  if (typeof event === 'string') return false
  if ('SpellCast' in event) return event.SpellCast.object === item
  if ('AbilityPutOnStack' in event) return event.AbilityPutOnStack.object === item
  if ('SpellCopied' in event) return event.SpellCopied.object === item
  if ('AbilityCopied' in event) return event.AbilityCopied.object === item
  return false
}

/**
 * The objects that left where they were since `item` went on the stack, read from the log backwards. Empty when the
 * log no longer holds that event: nothing is then claimed gone.
 */
export function movedSince(log: readonly LogEntry[], item: ObjectId): ReadonlySet<ObjectId> {
  const moved = new Set<ObjectId>()
  for (let index = log.length - 1; index >= 0; index -= 1) {
    const view = log[index]?.event
    if (view === undefined || !('Public' in view)) continue
    if (putsOnStack(view.Public, item)) return moved
    const id = leaver(view.Public)
    if (id !== null) moved.add(id)
  }
  return new Set()
}

/** What the view and the log say about the stack, for `stackEntries`. */
export interface StackSource {
  /** The stack, top first. */
  ids: readonly ObjectId[]
  objectsById: Readonly<Record<ObjectId, ObjectView>>
  players: readonly PlayerView[]
  log: readonly LogEntry[]
}

/**
 * Whether each target of `object` is legal by the server's word (`targets_legal`), by target key: one legal
 * occurrence is enough. Null when the view does not say, so the log stands in.
 */
export function legalByKey(object: ObjectView): ReadonlyMap<string, boolean> | null {
  const legal = object.targets_legal
  if (legal === undefined) return null
  const byKey = new Map<string, boolean>()
  ;(object.targets ?? []).forEach((set, spec) =>
    set.forEach((target, index) => {
      const key = targetKey(target)
      byKey.set(key, (byKey.get(key) ?? false) || legal[spec]?.[index] === true)
    }),
  )
  return byKey
}

/** The targets of each stack item, top first: every target once, a gone one marked. */
export function stackEntries({ ids, objectsById, players, log }: StackSource): StackEntry[] {
  const entries: StackEntry[] = []
  for (const id of ids) {
    const object = objectsById[id]
    if (object === undefined) continue
    const named = distinctTargets((object.targets ?? []).flat())
    const told = legalByKey(object)
    const moved =
      told === null && named.some((target) => 'Object' in target) ? movedSince(log, id) : new Set<ObjectId>()
    const targets = named.map((target): StackTarget => {
      const legal = told?.get(targetKey(target))
      if (legal !== undefined) {
        const aimed = 'Object' in target ? objectsById[target.Object.id] : undefined
        return {
          key: targetKey(target),
          target,
          gone: !legal,
          onStack: aimed?.zone === 'Stack',
          ...(aimed === undefined ? {} : { seat: aimed.controller }),
        }
      }
      if ('Player' in target) {
        const lost = players.find((player) => player.seat === target.Player)?.lost
        return { key: targetKey(target), target, gone: lost !== undefined && lost !== null, onStack: false }
      }
      const aimed = objectsById[target.Object.id]
      return {
        key: targetKey(target),
        target,
        gone: aimed === undefined || moved.has(target.Object.id),
        onStack: aimed?.zone === 'Stack',
        ...(aimed === undefined ? {} : { seat: aimed.controller }),
      }
    })
    entries.push({ id, controller: object.controller, targets })
  }
  return entries
}

/** The targets still legal. */
export const live = (entry: StackEntry): StackTarget[] => entry.targets.filter((target) => !target.gone)

/** Whether the item aims at something and nothing it aims at is still there. */
export const hasNoLegalTarget = (entry: StackEntry): boolean =>
  entry.targets.length > 0 && live(entry).length === 0
