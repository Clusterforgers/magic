/** What an Aura, Equipment or Fortification hangs on, read from a view and from the events that change it. Pure. */
import type { EventView, ObjectId, ObjectView, PlayerId } from '@/protocol/generated'

/** A permanent (`card`) or a player (`player`) an attachment hangs on. */
export type HostRef = { readonly card: ObjectId } | { readonly player: PlayerId }

/** The host of `object` as its view says, or null while it hangs on nothing. */
export function hostOf(object: ObjectView | undefined): HostRef | null {
  if (object?.attached_to !== undefined) return { card: object.attached_to }
  if (object?.attached_player !== undefined) return { player: object.attached_player }
  return null
}

/** Whether two hosts are the same permanent or the same player; two missing hosts are the same. */
export function sameHost(a: HostRef | null, b: HostRef | null): boolean {
  if (a === null || b === null) return a === b
  if ('card' in a) return 'card' in b && a.card === b.card
  return 'player' in b && a.player === b.player
}

/** What an event does to the host of one object: `to` is null when the object hangs on nothing afterwards. */
export interface HostChange {
  readonly object: ObjectId
  readonly to: HostRef | null
}

/**
 * The host change an event tells, or null for any other event. An object that enters or changes zone hangs on nothing
 * from then on, which the event of its attaching then replaces.
 */
export function hostChangeOf(view: EventView): HostChange | null {
  if (!('Public' in view) || typeof view.Public === 'string') return null
  const event = view.Public
  if ('Attached' in event) return { object: event.Attached.object, to: { card: event.Attached.to } }
  if ('AttachedToPlayer' in event) {
    return { object: event.AttachedToPlayer.object, to: { player: event.AttachedToPlayer.player } }
  }
  if ('Unattached' in event) return { object: event.Unattached.object, to: null }
  if ('Entered' in event) return { object: event.Entered.object, to: null }
  if ('ZoneChanged' in event) return { object: event.ZoneChanged.object, to: null }
  return null
}
