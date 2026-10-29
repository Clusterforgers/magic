/**
 * Which damage events of a run of the log are commander damage, with the total each brought the player to. The events
 * carry no combat flag, so combat damage is the damage dealt at the start of a combat damage step, before anybody gets
 * priority in it; the totals count back from the view's own totals, so a log cut short or joined late still adds up.
 */
import type { Event, EventView, ObjectId, PlayerId, PlayerView } from '@/protocol/generated'

/** One hit of commander damage. */
export interface CommanderHit {
  source: ObjectId
  player: PlayerId
  amount: number
  /** The damage from this commander to this player once the hit was dealt. */
  total: number
  /** The table's lethal amount, or undefined while the rule is off. */
  lethal: number | undefined
}

const publicOf = (view: EventView): Event | null => ('Public' in view ? view.Public : null)

/** Whether `step` deals combat damage at its start. */
const damageStep = (event: Event): boolean | null => {
  if (typeof event === 'string') return null
  if ('StepBegan' in event) {
    const { step } = event.StepBegan
    return step === 'CombatDamage' || step === 'FirstStrikeDamage'
  }
  if ('PriorityGranted' in event || 'PriorityPassed' in event || 'TurnBegan' in event) return false
  return null
}

/**
 * The commander damage among `events`, by index. `inDamage` says whether the run starts inside the dealing of combat
 * damage (a batch that began in it); `players` is the view after the last event.
 */
export function commanderHits(
  events: readonly EventView[],
  players: readonly PlayerView[],
  inDamage = false,
): Map<number, CommanderHit> {
  const commanders = new Set<ObjectId>(
    players.flatMap((player) => (player.commanders ?? []).map((commander) => commander.object)),
  )
  const hits = new Map<number, Omit<CommanderHit, 'total'>>()
  if (commanders.size === 0) return new Map()
  let dealing = inDamage
  events.forEach((view, index) => {
    const event = publicOf(view)
    if (event === null) return
    const step = damageStep(event)
    if (step !== null) dealing = step
    if (!dealing || typeof event === 'string' || !('DamageDealt' in event)) return
    const { source, target, amount } = event.DamageDealt
    if (!commanders.has(source) || !('Player' in target)) return
    const player = players.find((candidate) => candidate.seat === target.Player)
    hits.set(index, { source, player: target.Player, amount, lethal: player?.commander_lethal })
  })
  // The view's totals are the totals after the last hit; each earlier hit is that less what came after it.
  const after = new Map<string, number>()
  const result = new Map<number, CommanderHit>()
  for (const [index, hit] of [...hits].reverse()) {
    const key = `${hit.player}:${hit.source}`
    const now =
      players
        .find((player) => player.seat === hit.player)
        ?.commander_damage?.find((entry) => entry.commander === hit.source)?.damage ?? 0
    const later = after.get(key) ?? 0
    result.set(index, { ...hit, total: Math.max(hit.amount, now - later) })
    after.set(key, later + hit.amount)
  }
  return new Map([...result].reverse())
}
