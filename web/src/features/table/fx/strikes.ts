/**
 * Combat damage as strikes (docs/web.md, Table feedback): which creature strikes what in a batch, and the path its
 * picture takes there and back. Pure: `planStrikes` reads the public events and the two moments, `strikeKeyframes`
 * turns two boxes into a path, `impactDelay` says when a strike lands.
 */
import type { CombatView, EventView, ObjectId, PlayerId, Step } from '@/protocol/generated'
import type { Moment } from './derive'
import type { Effect } from './model'
import type { Box } from './places'

/** What a strike lands on: a permanent (a creature, planeswalker or battle) or a player's life total. */
export type StrikeTarget = { readonly object: ObjectId } | { readonly player: PlayerId }

/** One packet of combat damage a strike carries. */
export interface StrikeHit {
  readonly target: StrikeTarget
  readonly amount: number
}

/** One creature's combat damage in one damage step: it lunges at the first of its hits and every hit lands at once. */
export interface StrikePlan {
  readonly source: ObjectId
  /** Its hits, in the order the events dealt them; summed per target. */
  readonly hits: readonly StrikeHit[]
  /** The damage step of the batch it belongs to: 0 for the first, 1 for regular damage after first strike. */
  readonly wave: number
  /** The first hit is a creature that strikes back in the same wave, so the two meet halfway. */
  readonly mutual: boolean
}

/** How long a strike takes there and back, in ms (times the slow-down). */
export const STRIKE_MS = 1500

/** The share of a strike at which it lands on its target. */
export const STRIKE_HIT = 0.5

/** The most strikes one batch plays. */
export const MAX_STRIKES = 40

/** The gap a strike stops short of its target's edge, in px. */
export const STRIKE_GAP_PX = 6

/** How far a strike draws back before it lunges, in px. */
export const STRIKE_WINDUP_PX = 16

/** How far a strike bounces back off its target, in px. */
export const STRIKE_RECOIL_PX = 18

const DAMAGE_STEPS: readonly Step[] = ['FirstStrikeDamage', 'CombatDamage']

/** When the hits of `wave` land, in ms after the batch. */
export const impactDelay = (wave: number, slow = 1): number => (wave + STRIKE_HIT) * STRIKE_MS * slow

const combatantsOf = (combat: CombatView): ObjectId[] =>
  combat.attackers.flatMap((attacker) => [attacker.object, ...attacker.blockers])

const sameTarget = (a: StrikeTarget, b: StrikeTarget): boolean =>
  'object' in a ? 'object' in b && a.object === b.object : 'player' in b && a.player === b.player

/**
 * The strikes of a batch. Combat damage is a turn-based action at the start of a damage step, before anyone has
 * priority (CR 510.2), so a `DamageDealt` counts when it follows the `StepBegan` of a damage step (or `DamageAssigned`,
 * or the batch began in a damage step nobody had priority in) with no `PriorityGranted`, other step or resolution
 * between, and its source attacks or blocks in the combat before, after or declared in the batch. Damage a spell or
 * ability deals never strikes.
 */
export function planStrikes(before: Moment, after: Moment, events: readonly EventView[]): StrikePlan[] {
  const fighters = new Set([...combatantsOf(before.view.combat), ...combatantsOf(after.view.combat)])
  for (const view of events) {
    if (!('Public' in view) || typeof view.Public === 'string') continue
    const event = view.Public
    if ('AttackersDeclared' in event) for (const [id] of event.AttackersDeclared.attackers) fighters.add(id)
    if ('BlockersDeclared' in event) for (const [id] of event.BlockersDeclared.blocks) fighters.add(id)
  }
  let open = DAMAGE_STEPS.includes(before.view.turn.step) && before.view.priority === null
  let wave = 0
  let waveUsed = false
  const plans: { source: ObjectId; hits: StrikeHit[]; wave: number }[] = []
  for (const view of events) {
    if (!('Public' in view)) continue
    const event = view.Public
    if (typeof event === 'string') continue
    if ('StepBegan' in event) {
      open = DAMAGE_STEPS.includes(event.StepBegan.step)
      if (open && waveUsed) {
        wave += 1
        waveUsed = false
      }
    } else if ('PriorityGranted' in event || 'ResolutionBegan' in event) {
      open = false
    } else if ('DamageAssigned' in event) {
      open = true
    } else if ('DamageDealt' in event && open) {
      const { source, target, amount } = event.DamageDealt
      if (!fighters.has(source) || amount <= 0) continue
      const hitTarget: StrikeTarget =
        'Player' in target ? { player: target.Player } : { object: target.Object }
      let plan = plans.find((candidate) => candidate.source === source && candidate.wave === wave)
      if (plan === undefined) {
        plan = { source, hits: [], wave }
        plans.push(plan)
      }
      waveUsed = true
      const index = plan.hits.findIndex((hit) => sameTarget(hit.target, hitTarget))
      const was = plan.hits[index]
      if (was === undefined) plan.hits.push({ target: hitTarget, amount })
      else plan.hits[index] = { target: hitTarget, amount: was.amount + amount }
    }
  }
  const picked = plans.slice(0, MAX_STRIKES)
  return picked.map((plan) => {
    const first = plan.hits[0]?.target
    const mutual =
      first !== undefined &&
      'object' in first &&
      picked.some(
        (other) =>
          other.source === first.object &&
          other.wave === plan.wave &&
          other.hits[0] !== undefined &&
          sameTarget(other.hits[0].target, { object: plan.source }),
      )
    return { ...plan, mutual }
  })
}

/** The struck permanents and seats of `plans`, with the delay each one's impact lands after. */
export function impactsOf(
  plans: readonly StrikePlan[],
  slow = 1,
): { objects: Map<ObjectId, number>; seats: Map<PlayerId, number> } {
  const objects = new Map<ObjectId, number>()
  const seats = new Map<PlayerId, number>()
  for (const plan of plans) {
    const delay = impactDelay(plan.wave, slow)
    for (const { target } of plan.hits) {
      const map: Map<number, number> = 'object' in target ? objects : seats
      const key = 'object' in target ? target.object : target.player
      map.set(key, Math.max(map.get(key) ?? 0, delay))
    }
  }
  return { objects, seats }
}

/**
 * The effects of a batch split by when they play: `now`, and `later` by delay in ms for the `damaged` effect of each
 * struck permanent or seat, so its flash and number land with the strike.
 */
export function splitImpacts(
  effects: readonly Effect[],
  impacts: { objects: ReadonlyMap<ObjectId, number>; seats: ReadonlyMap<PlayerId, number> },
): { now: Effect[]; later: Map<number, Effect[]> } {
  const now: Effect[] = []
  const later = new Map<number, Effect[]>()
  for (const effect of effects) {
    const delay =
      effect.kind !== 'damaged'
        ? undefined
        : 'seat' in effect
          ? impacts.seats.get(effect.seat)
          : impacts.objects.get(effect.object)
    if (delay === undefined) now.push(effect)
    else later.set(delay, [...(later.get(delay) ?? []), effect])
  }
  return { now, later }
}

/**
 * The batch's deaths held until combat is done with them: a struck creature dies once the last strike on it lands, and
 * a creature that strikes and dies once it is back where it stood, so its ghost is there to return to.
 */
export function holdDeaths(effects: readonly Effect[], plans: readonly StrikePlan[], slow = 1): Effect[] {
  const { objects } = impactsOf(plans, slow)
  const holds = new Map(objects)
  for (const plan of plans) {
    const back = (plan.wave + 1) * STRIKE_MS * slow
    holds.set(plan.source, Math.max(holds.get(plan.source) ?? 0, back))
  }
  return effects.map((effect) => {
    const hold = effect.kind === 'died' ? holds.get(effect.object) : undefined
    return hold === undefined ? effect : { ...effect, hold }
  })
}

const centerOf = (box: Box): { x: number; y: number } => ({
  x: box.x + box.width / 2,
  y: box.y + box.height / 2,
})

/** How far from its centre a box's edge is along the unit direction (dx, dy). */
const reachOf = (box: Box, dx: number, dy: number): number => {
  const alongX = dx === 0 ? Infinity : box.width / 2 / Math.abs(dx)
  const alongY = dy === 0 ? Infinity : box.height / 2 / Math.abs(dy)
  return Math.min(alongX, alongY)
}

/**
 * The keyframes of a strike for a picture laid over `from`: it lifts and draws back, lunges until its edge meets the
 * target's (halfway when the two strike each other), recoils a little and glides back to where it stood. The picture's
 * origin is its top left corner, placed at `from`; every scale is about the picture's centre.
 */
export function strikeKeyframes(from: Box, to: Box, mutual = false): Keyframe[] {
  const a = centerOf(from)
  const b = centerOf(to)
  const distance = Math.hypot(b.x - a.x, b.y - a.y)
  const dx = distance === 0 ? 0 : (b.x - a.x) / distance
  const dy = distance === 0 ? 0 : (b.y - a.y) / distance
  const gap = Math.max(0, distance - reachOf(from, dx, dy) - reachOf(to, dx, dy) - STRIKE_GAP_PX)
  const travel = mutual ? gap / 2 : gap
  const recoil = Math.min(STRIKE_RECOIL_PX, travel)
  const at = (step: number, scale: number): string => {
    const x = from.x + dx * step - (from.width * (scale - 1)) / 2
    const y = from.y + dy * step - (from.height * (scale - 1)) / 2
    return `translate(${Number(x.toFixed(1))}px, ${Number(y.toFixed(1))}px) scale(${scale})`
  }
  return [
    { offset: 0, transform: at(0, 1), easing: 'cubic-bezier(0.45, 0, 0.55, 1)' },
    { offset: 0.22, transform: at(-STRIKE_WINDUP_PX, 1.1), easing: 'cubic-bezier(0.4, 0, 0.9, 0.5)' },
    { offset: STRIKE_HIT, transform: at(travel, 1.12), easing: 'cubic-bezier(0.2, 0.7, 0.4, 1)' },
    { offset: 0.6, transform: at(travel - recoil, 1.1), easing: 'cubic-bezier(0.45, 0, 0.55, 1)' },
    { offset: 1, transform: at(0, 1) },
  ]
}
