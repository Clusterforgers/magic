/** What a log line needs from the events around it: the object resolving at that point and the targets an activation names. */
import type { Event, EventView, ObjectId, Target } from '@/protocol/generated'

/** The surroundings of one event; a line that needs none of them reads the same without. */
export interface EventContext {
  /** The spell or ability that was resolving when the event happened: the last `ResolutionBegan` not yet ended. */
  resolving?: ObjectId
  /** For an activation: the targets of the ability that went on the stack from the same source right after it. */
  targets?: readonly Target[]
  /** For an answer by option index: what it answered when a coin flip follows it (the call, then which coin to keep). */
  coin?: CoinAnswer
}

/** The question of a flip an option answered: the call of the coin or which of two coins to keep (Krark's Thumb). */
export type CoinAnswer = 'call' | 'keep'

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
 * What the option answer at `index` answered, when the run of option answers it belongs to ends in a coin flip of the
 * same player: the first of the run is the call, the next the coin kept. Any other answer is not a flip's.
 */
function coinAnswer(events: readonly EventView[], index: number, player: number): CoinAnswer | undefined {
  const eventAt = (at: number): Event | null => {
    const view = events[at]
    return view === undefined ? null : publicEvent(view)
  }
  const isOption = (event: Event | null): boolean =>
    event !== null &&
    typeof event !== 'string' &&
    'ChoiceMade' in event &&
    event.ChoiceMade.player === player &&
    'Option' in event.ChoiceMade.choice
  let first = index
  while (first > 0 && isOption(eventAt(first - 1))) first -= 1
  let next = index + 1
  while (next < events.length && isOption(eventAt(next))) next += 1
  const after = eventAt(next)
  if (after === null || typeof after === 'string' || !('CoinFlipped' in after)) return undefined
  if (after.CoinFlipped.player !== player) return undefined
  const at = index - first
  return at === 0 ? 'call' : at === 1 ? 'keep' : undefined
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
    if ('ChoiceMade' in event && 'Option' in event.ChoiceMade.choice) {
      const coin = coinAnswer(events, index, event.ChoiceMade.player)
      if (coin !== undefined) return by === undefined ? { coin } : { resolving: by, coin }
    }
    return by === undefined ? undefined : { resolving: by }
  })
}

/**
 * The card each ability object of `events` comes from (a trigger or activation put on the stack, a copy of one), so a
 * line about the ability resolving names the card it belongs to even after the object left the view.
 */
export function abilitySources(events: readonly EventView[]): ReadonlyMap<ObjectId, ObjectId> {
  const sources = new Map<ObjectId, ObjectId>()
  for (const view of events) {
    const event = publicEvent(view)
    if (event === null || typeof event === 'string') continue
    if ('AbilityPutOnStack' in event) {
      const { object, source } = event.AbilityPutOnStack
      sources.set(object, source.id)
    } else if ('AbilityCopied' in event) {
      const { object, of } = event.AbilityCopied
      sources.set(object, sources.get(of.id) ?? of.id)
    }
  }
  return sources
}
