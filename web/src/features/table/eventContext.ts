/** What a log line needs from the events around it: the object resolving at that point and the targets an activation names. */
import type { Event, EventView, ObjectId, Target } from '@/protocol/generated'

/** The surroundings of one event; a line that needs none of them reads the same without. */
export interface EventContext {
  /** The spell or ability that was resolving when the event happened: the last `ResolutionBegan` not yet ended. */
  resolving?: ObjectId
  /** For an activation: the targets of the ability that went on the stack from the same source right after it. */
  targets?: readonly Target[]
}

/** How far after an activation its ability goes on the stack (costs are paid in between). */
const LOOKAHEAD = 24

const publicEvent = (view: EventView): Event | null => ('Public' in view ? view.Public : null)

/** The targets of the ability `source` put on the stack, looking at the events after `from`. */
function activatedTargets(
  events: readonly EventView[],
  from: number,
  source: ObjectId,
): readonly Target[] | undefined {
  for (let index = from + 1; index < Math.min(events.length, from + 1 + LOOKAHEAD); index += 1) {
    const view = events[index]
    const event = view === undefined ? null : publicEvent(view)
    if (event === null || typeof event === 'string') continue
    if ('AbilityPutOnStack' in event) {
      const put = event.AbilityPutOnStack
      if (put.source.id === source && put.cause === undefined) return put.choices?.targets?.flat()
    }
    // The next decision of the game ends the search.
    if ('ResolutionBegan' in event || 'AbilityActivated' in event || 'PriorityGranted' in event)
      return undefined
  }
  return undefined
}

/**
 * The context of every event of `events`, index for index. Pure: it reads only the events, so the same events give the
 * same lines whichever seat reads them.
 */
export function eventContexts(events: readonly EventView[]): (EventContext | undefined)[] {
  const resolving: ObjectId[] = []
  return events.map((view, index) => {
    const event = publicEvent(view)
    if (event === null || typeof event === 'string') return undefined
    if ('ResolutionBegan' in event) {
      resolving.push(event.ResolutionBegan.object)
      return undefined
    }
    if ('ResolutionEnded' in event) {
      const { object } = event.ResolutionEnded
      const at = resolving.lastIndexOf(object)
      // An object ended by a counter is not the one resolving: only the end of the resolving one pops it.
      const by = resolving.at(-1)
      if (at >= 0) resolving.splice(at, 1)
      return by === undefined || by === object ? undefined : { resolving: by }
    }
    if ('AbilityActivated' in event) {
      const targets = activatedTargets(events, index, event.AbilityActivated.source.id)
      return targets === undefined || targets.length === 0 ? undefined : { targets }
    }
    const by = resolving.at(-1)
    return by === undefined ? undefined : { resolving: by }
  })
}
