/** What an Aura, Equipment or Fortification hangs on, read from a view, and where it hung before an attach event. Pure. */
import type { EventView, ObjectId, ObjectView, PlayerId } from '@/protocol/generated'

/** A permanent (`card`) or a player (`player`) an attachment hangs on. */
export type HostRef = { readonly card: ObjectId } | { readonly player: PlayerId }

/** The host of `object` as its view says, or null while it hangs on nothing. */
export function hostOf(object: ObjectView | undefined): HostRef | null {
  if (object?.attached_to !== undefined) return { card: object.attached_to }
  if (object?.attached_player !== undefined) return { player: object.attached_player }
  return null
}

/** An Equipment or Fortification on the battlefield that hangs on no permanent: it waits to be attached. */
export function isUnattached(object: ObjectView): boolean {
  return (
    object.zone === 'Battlefield' &&
    object.attached_to === undefined &&
    object.subtypes.some((subtype) => subtype === 'Equipment' || subtype === 'Fortification')
  )
}

/** Whether two hosts are the same permanent or the same player; two missing hosts are the same. */
export function sameHost(a: HostRef | null, b: HostRef | null): boolean {
  if (a === null || b === null) return a === b
  if ('card' in a) return 'card' in b && a.card === b.card
  return 'player' in b && a.player === b.player
}

/**
 * Where the attachment of each attach event hung just before that event, read from the objects before the batch and
 * the earlier events of the batch itself: a host, null for none (an object the viewer did not know hung on nothing),
 * and undefined for an event that attaches nothing.
 */
export function hostsBefore(
  events: readonly EventView[],
  objects: Readonly<Record<ObjectId, ObjectView>>,
): Array<HostRef | null | undefined> {
  const moved = new Map<ObjectId, HostRef | null>()
  return events.map((view) => {
    if (!('Public' in view) || typeof view.Public === 'string') return undefined
    const event = view.Public
    if ('Unattached' in event) {
      moved.set(event.Unattached.object, null)
      return undefined
    }
    const [object, to]: [ObjectId, HostRef] | [] =
      'Attached' in event
        ? [event.Attached.object, { card: event.Attached.to }]
        : 'AttachedToPlayer' in event
          ? [event.AttachedToPlayer.object, { player: event.AttachedToPlayer.player }]
          : []
    if (object === undefined || to === undefined) return undefined
    const was = moved.has(object) ? (moved.get(object) ?? null) : hostOf(objects[object])
    moved.set(object, to)
    return was
  })
}
