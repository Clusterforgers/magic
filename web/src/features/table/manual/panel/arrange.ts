/** Pure: the arrangement of the top cards a seat looked at by hand, from where each goes to the action it sends and the words of the button. */
import type { LogEntry } from '@/play/store'
import type { ManualAction, ObjectId, PlayerId } from '@/protocol/generated'

/** Where a card goes. */
export type Destination = 'top' | 'bottom' | 'graveyard'

/** The destinations in the order the list shows them, with the words of each. */
export const DESTINATIONS: readonly { id: Destination; label: string; heading: string }[] = [
  { id: 'top', label: 'Top', heading: 'Stays on top' },
  { id: 'bottom', label: 'Bottom', heading: 'Goes to the bottom' },
  { id: 'graveyard', label: 'Graveyard', heading: 'Goes to the graveyard' },
]

/** The cards by destination, each list in the order it will be put back: top first, and for the bottom the last card ends lowest. */
export interface Arrangement {
  top: readonly ObjectId[]
  bottom: readonly ObjectId[]
  graveyard: readonly ObjectId[]
}

/** Every card on top, in the order looked at. */
export function startArrangement(looked: readonly ObjectId[]): Arrangement {
  return { top: looked, bottom: [], graveyard: [] }
}

/** Where `id` goes now, or undefined for a card the arrangement does not hold. */
export function destinationOf(arrangement: Arrangement, id: ObjectId): Destination | undefined {
  return DESTINATIONS.find((place) => arrangement[place.id].includes(id))?.id
}

/** `id` sent to `to`, at the end of that list; the same arrangement when it is there already or is not held. */
export function sendTo(arrangement: Arrangement, id: ObjectId, to: Destination): Arrangement {
  const from = destinationOf(arrangement, id)
  if (from === undefined || from === to) return arrangement
  return {
    ...arrangement,
    [from]: arrangement[from].filter((card) => card !== id),
    [to]: [...arrangement[to], id],
  }
}

/** `id` one place up (`-1`) or down (`1`) in its list; the same arrangement at either end. */
export function moveWithin(arrangement: Arrangement, id: ObjectId, step: -1 | 1): Arrangement {
  const place = destinationOf(arrangement, id)
  if (place === undefined) return arrangement
  const list = [...arrangement[place]]
  const at = list.indexOf(id)
  const to = at + step
  if (to < 0 || to >= list.length) return arrangement
  const swapped = list[to]
  if (swapped === undefined) return arrangement
  list[to] = id
  list[at] = swapped
  return { ...arrangement, [place]: list }
}

/** Whether the cards go back exactly as they were looked at: all on top in the same order. */
export function isUnchanged(arrangement: Arrangement, looked: readonly ObjectId[]): boolean {
  return (
    arrangement.bottom.length === 0 &&
    arrangement.graveyard.length === 0 &&
    arrangement.top.length === looked.length &&
    arrangement.top.every((card, at) => card === looked[at])
  )
}

/** How the engine reads the arrangement: a surveil when nothing goes to the bottom but something to the graveyard, a scry when anything goes to the bottom, else a reorder. */
export function kindOf(arrangement: Arrangement): 'surveil' | 'scry' | 'reorder' {
  if (arrangement.bottom.length > 0) return 'scry'
  return arrangement.graveyard.length > 0 ? 'surveil' : 'reorder'
}

/** The `ArrangeTop` that puts exactly the looked-at cards back as arranged. */
export function arrangeAction(arrangement: Arrangement): ManualAction {
  return {
    ArrangeTop: {
      top: [...arrangement.top],
      bottom: [...arrangement.bottom],
      graveyard: [...arrangement.graveyard],
    },
  }
}

/** The words of the Apply button: "Put 2 on the bottom, 1 in the graveyard"; "Keep all 3 on top" for no change, "Reorder the 3 on top" for a new order only. */
export function applyWords(arrangement: Arrangement, looked: readonly ObjectId[]): string {
  if (isUnchanged(arrangement, looked)) {
    return looked.length === 1 ? 'Keep it on top' : `Keep all ${looked.length} on top`
  }
  const parts = [
    arrangement.bottom.length > 0 ? `${arrangement.bottom.length} on the bottom` : null,
    arrangement.graveyard.length > 0 ? `${arrangement.graveyard.length} in the graveyard` : null,
  ].filter((part) => part !== null)
  return parts.length === 0 ? `Reorder the ${arrangement.top.length} on top` : `Put ${parts.join(', ')}`
}

/** The preview line, in the seat's words, with the name of each card: "Scry: A and B stay on top; C goes to the bottom". */
export function previewWords(
  arrangement: Arrangement,
  looked: readonly ObjectId[],
  nameOf: (id: ObjectId) => string,
): string {
  if (isUnchanged(arrangement, looked)) return 'Nothing moves: the cards stay on top in this order.'
  const list = (ids: readonly ObjectId[]): string => ids.map(nameOf).join(', ')
  const verb = (ids: readonly ObjectId[], one: string, many: string): string =>
    ids.length === 1 ? one : many
  const said = [
    arrangement.top.length === 0
      ? null
      : `${list(arrangement.top)} ${verb(arrangement.top, 'stays', 'stay')} on top`,
    arrangement.bottom.length === 0
      ? null
      : `${list(arrangement.bottom)} ${verb(arrangement.bottom, 'goes', 'go')} to the bottom`,
    arrangement.graveyard.length === 0
      ? null
      : `${list(arrangement.graveyard)} ${verb(arrangement.graveyard, 'goes', 'go')} to the graveyard`,
  ].filter((part) => part !== null)
  const heading = { surveil: 'Surveil', scry: 'Scry', reorder: 'Reorder' }[kindOf(arrangement)]
  return `${heading}: ${said.join('; ')}.`
}

/** What the log says of the cards the seat's own manual actions looked at and put back, newest last: true while the last of them is a look. */
export function lookIsPending(log: readonly LogEntry[], seat: PlayerId): boolean {
  for (let at = log.length - 1; at >= 0; at -= 1) {
    const entry = log[at]
    if (entry === undefined || !('Public' in entry.event)) continue
    const event = entry.event.Public
    if (typeof event === 'string' || !('ManualAction' in event) || event.ManualAction.player !== seat)
      continue
    const { action } = event.ManualAction
    if ('LookTop' in action) return true
    if ('ArrangeTop' in action || 'Shuffle' in action) return false
  }
  return false
}
