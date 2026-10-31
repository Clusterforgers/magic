/** Whether the seat owes the table the clauses of its own resolved spell or ability, read from the events as the server reads them. Pure. */
import type { Event, EventView, ObjectId, ObjectView, PlayerId } from '@/protocol/generated'

type Objects = Readonly<Record<ObjectId, ObjectView>>

/** An object of the stack whose resolution may leave clauses to the table: who controls it and whether its card has any. */
interface Owed {
  controller: PlayerId
  clauses: boolean
}

const owed = ({ controller, manual }: ObjectView): Owed => ({
  controller,
  clauses: (manual?.length ?? 0) > 0,
})

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
 * Whether `seat` owes its by-hand clauses after `events`, given whether it did before them: the server holds a seat at
 * every priority window from the resolution of an object of its own whose rules text has clauses left to the table
 * (`ObjectView.manual`) until it passes priority, and never tells it. The object is read from the view before the batch
 * (it was on the stack) or, for a card that kept its id, after it; an ability put on the stack and resolved inside the
 * batch is in neither, and has the clauses of its source's card.
 */
export function owesByHand(
  owes: boolean,
  events: readonly EventView[],
  seat: PlayerId | null,
  before: Objects,
  after: Objects,
): boolean {
  if (seat === null) return false
  const abilities = new Map<ObjectId, Owed>()
  let held = owes
  for (const view of events) {
    if (!('Public' in view) || typeof view.Public === 'string') continue
    const event = view.Public
    if ('PriorityPassed' in event) {
      if (event.PriorityPassed.player === seat) held = false
      continue
    }
    if ('AbilityPutOnStack' in event) {
      const { object, source, controller } = event.AbilityPutOnStack
      const from = before[source.id] ?? after[source.id]
      if (from !== undefined) abilities.set(object, { controller, clauses: owed(from).clauses })
      continue
    }
    const id = resolved(event)
    if (id === null) continue
    const known = before[id] ?? after[id]
    const found = known === undefined ? abilities.get(id) : owed(known)
    if (found?.controller === seat && found.clauses) held = true
  }
  return held
}
