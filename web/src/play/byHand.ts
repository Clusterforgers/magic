/** Which card holds the seat at its by-hand window, read from the events and the server's `ObjectView.manual_owed`. Pure. */
import type { Event, EventView, ObjectId, ObjectView, PlayerId } from '@/protocol/generated'

type Objects = Readonly<Record<ObjectId, ObjectView>>

/** An object of the stack whose resolution may leave clauses to the table: who controls it and, when it does, its card's name. */
interface Owed {
  controller: PlayerId
  card: string | null
}

/** The server says whether the object's card is owed at resolution (`manual_owed`); the client never reads the clauses. */
const owed = ({ controller, manual_owed, name }: ObjectView): Owed => ({
  controller,
  card: manual_owed === true ? name : null,
})

/** What the seat is told when the source of an ability the game left to the table is out of its view. */
export const UNSEEN_SOURCE = 'a card'

/** The source and controller of an ability the game could not run (`AbilityLeftToTable`, public or hidden), or null. */
function leftToTable(view: EventView): { source: ObjectId; controller: PlayerId } | null {
  if ('Hidden' in view) {
    const hidden = view.Hidden
    if (typeof hidden === 'string' || !('AbilityLeftToTable' in hidden)) return null
    return { source: hidden.AbilityLeftToTable.source.id, controller: hidden.AbilityLeftToTable.controller }
  }
  const event = view.Public
  if (typeof event === 'string' || !('AbilityLeftToTable' in event)) return null
  return { source: event.AbilityLeftToTable.source.id, controller: event.AbilityLeftToTable.controller }
}

/** The object a public event says resolved, or null for any other event. */
function resolved(event: Event): ObjectId | null {
  if (typeof event === 'string') return null
  if ('SpellResolved' in event) return event.SpellResolved.object
  if ('ResolutionEnded' in event && event.ResolutionEnded.outcome === 'Resolved') {
    return event.ResolutionEnded.object
  }
  return null
}

/**
 * The name of the card `seat` owes its by-hand clauses for after `events`, or null, given the card it owed before them:
 * the server holds a seat at every priority window from the resolution of an object of its own whose card has a clause
 * owed at resolution (`ObjectView.manual_owed`) until it passes priority, and never tells it. The object is read from the
 * view before the batch (it was on the stack) or, for a card that kept its id, after it; an ability put on the stack and
 * resolved inside the batch is in neither, and takes the flag of its source's view. An ability of the seat's that the
 * game left to the table (`AbilityLeftToTable`) and a by-hand line of its own that came due (`ByHandDue`) hold it too,
 * for their source card. A later resolution names the card.
 */
export function owesByHand(
  owes: string | null,
  events: readonly EventView[],
  seat: PlayerId | null,
  before: Objects,
  after: Objects,
): string | null {
  if (seat === null) return null
  const abilities = new Map<ObjectId, Owed>()
  let held = owes
  for (const view of events) {
    const left = leftToTable(view)
    if (left !== null) {
      if (left.controller === seat) held = (before[left.source] ?? after[left.source])?.name ?? UNSEEN_SOURCE
      continue
    }
    if (!('Public' in view) || typeof view.Public === 'string') continue
    const event = view.Public
    if ('ByHandDue' in event) {
      const { source, controller } = event.ByHandDue
      if (controller === seat) held = (before[source.id] ?? after[source.id])?.name ?? UNSEEN_SOURCE
      continue
    }
    if ('PriorityPassed' in event) {
      if (event.PriorityPassed.player === seat) held = null
      continue
    }
    if ('AbilityPutOnStack' in event) {
      const { object, source, controller } = event.AbilityPutOnStack
      const from = before[source.id] ?? after[source.id]
      if (from !== undefined) abilities.set(object, { controller, card: owed(from).card })
      continue
    }
    const id = resolved(event)
    if (id === null) continue
    const known = before[id] ?? after[id]
    const found = known === undefined ? abilities.get(id) : owed(known)
    if (found?.controller === seat && found.card !== null) held = found.card
  }
  return held
}

/** A by-hand line of the seat's that came due (`ByHandDue`): its card's name and the line's words. */
export interface DueLine {
  card: string
  text: string
}

/**
 * The seat's due lines after `events`, given `lines` before them: a by-hand line of its own that came due joins them, in
 * the order of the log, and a pass of the seat's priority clears them, as it ends the by-hand window. Pure.
 */
export function dueLinesAfter(
  lines: readonly DueLine[],
  events: readonly EventView[],
  seat: PlayerId | null,
  before: Objects,
  after: Objects,
): readonly DueLine[] {
  if (seat === null) return lines.length === 0 ? lines : []
  let held = lines
  for (const view of events) {
    if (!('Public' in view) || typeof view.Public === 'string') continue
    const event = view.Public
    if ('PriorityPassed' in event && event.PriorityPassed.player === seat && held.length > 0) held = []
    if ('ByHandDue' in event && event.ByHandDue.controller === seat) {
      const { source, text } = event.ByHandDue
      const card = (before[source.id] ?? after[source.id])?.name ?? UNSEEN_SOURCE
      held = [...held, { card, text }]
    }
  }
  return held
}
