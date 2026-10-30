/** Pure: the names of cards that have left the view, kept so the log still says which card a line is about: those a manual look, put-back or reveal tells stay with their entry, the rest of the objects that departed are kept for the whole log. */
import type { EventView, LogName, ObjectId, ObjectView } from '@/protocol/generated'

type Objects = Readonly<Record<ObjectId, ObjectView>>
type Names = Readonly<Record<ObjectId, string>>

/** How many names of objects that left the view `logNames` keeps; the newest objects stay. */
export const LOG_NAMES_LIMIT = 1000

function newest(names: Record<ObjectId, string>): Names {
  const ids = Object.keys(names).map(Number)
  if (ids.length <= LOG_NAMES_LIMIT) return names
  const kept: Record<ObjectId, string> = {}
  for (const id of ids.sort((a, b) => a - b).slice(-LOG_NAMES_LIMIT)) kept[id] = names[id] ?? ''
  return kept
}

/** `held` with the names a snapshot brought for the objects its log window mentions that its state no longer shows. */
export function mergeLogNames(held: Names, given: readonly LogName[] = []): Names {
  if (given.length === 0) return held
  const merged: Record<ObjectId, string> = { ...held }
  for (const { object, name } of given) merged[object] = name
  return newest(merged)
}

/** `held` with the names of the objects a batch took from the view (`gone`), read from the objects before it. */
export function rememberDeparted(held: Names, before: Objects, gone: readonly ObjectId[]): Names {
  const merged: Record<ObjectId, string> = { ...held }
  let added = false
  for (const id of gone) {
    const name = before[id]?.name
    if (name === undefined || name === '' || merged[id] === name) continue
    merged[id] = name
    added = true
  }
  return added ? newest(merged) : held
}

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
