/** Pure: the names of the cards a manual look, put-back or reveal tells, kept with the log entry so the line still names a card that has since left the view. */
import type { EventView, ObjectId, ObjectView } from '@/protocol/generated'

type Objects = Readonly<Record<ObjectId, ObjectView>>

/** The cards a public event names that the view stops showing once they move: looked at, put back or revealed. */
export function idsToRemember(event: EventView): ObjectId[] {
  if (!('Public' in event) || typeof event.Public === 'string') return []
  const inner = event.Public
  if ('LibraryLooked' in inner) return inner.LibraryLooked.cards
  if ('Revealed' in inner) return inner.Revealed.cards
  if ('ManualAction' in inner) {
    const { action } = inner.ManualAction
    if ('ArrangeTop' in action) {
      const { top, bottom, graveyard } = action.ArrangeTop
      return [...top, ...bottom, ...graveyard]
    }
    if ('Reveal' in action && 'Hand' in action.Reveal.what) return [action.Reveal.what.Hand.object]
  }
  return []
}

/** The names of the cards `event` tells, from the objects after its batch or else before it; undefined when it tells none. */
export function namesToRemember(
  event: EventView,
  before: Objects | undefined,
  after: Objects | undefined,
): Readonly<Record<ObjectId, string>> | undefined {
  const named: Record<ObjectId, string> = {}
  for (const id of idsToRemember(event)) {
    const name = after?.[id]?.name || before?.[id]?.name
    if (name !== undefined && name !== '') named[id] = name
  }
  return Object.keys(named).length === 0 ? undefined : named
}
