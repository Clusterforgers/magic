/**
 * Which decorations a batch of events deserves, worked out from the game before it, the game after it and the events
 * it carried. Pure: nothing here reads the DOM, a clock or the store. The batch's object diff is not passed along,
 * since `applyBatch` keeps the identity of every object it did not change; comparing the two moments finds the same
 * objects (and, by value, finds them even when a test builds the two moments apart).
 *
 * The events say what happened inside a batch that the two moments cannot: a whole combat can run in one batch
 * (nobody had a decision to make), leaving no attackers behind. The moments say what the events do not: life totals
 * and damage marked have no event of their own.
 */
import { sameJson } from '@/play/view'
import type { CombatView, EventView, ObjectId, ObjectView, PlayerId, StateView } from '@/protocol/generated'
import { rowOf } from '../cardModel'
import { hostOf, sameHost } from '../hosts'
import type { HostRef } from '../hosts'
import { splitBattlefield } from '../selectors'
import type { DiedEffect, Effect } from './model'

type Objects = Readonly<Record<ObjectId, ObjectView>>

/** The game at one moment, as the play store keeps it. */
export interface Moment {
  readonly view: StateView
  readonly objectsById: Objects
}

/** The most permanents one batch leaves a ghost for; a board wipe at a crowded table stays cheap to draw. */
export const MAX_GHOSTS = 24

interface Facts {
  attackers: Set<ObjectId>
  blockers: Set<ObjectId>
  /** Damage dealt to each object, summed over the batch. */
  damage: Map<ObjectId, number>
  /** Creatures that state-based actions put into a graveyard. */
  dead: Set<ObjectId>
}

/** What the public events of a batch say about combat, damage and death. Hidden events name nothing visible. */
function factsOf(events: readonly EventView[]): Facts {
  const facts: Facts = { attackers: new Set(), blockers: new Set(), damage: new Map(), dead: new Set() }
  for (const view of events) {
    if (!('Public' in view)) continue
    const event = view.Public
    if (typeof event === 'string') continue
    if ('AttackersDeclared' in event) {
      for (const [attacker] of event.AttackersDeclared.attackers) facts.attackers.add(attacker)
    } else if ('BlockersDeclared' in event) {
      for (const [blocker] of event.BlockersDeclared.blocks) facts.blockers.add(blocker)
    } else if ('DamageDealt' in event) {
      const { target, amount } = event.DamageDealt
      if ('Object' in target) facts.damage.set(target.Object, (facts.damage.get(target.Object) ?? 0) + amount)
    } else if ('CreatureDied' in event) {
      facts.dead.add(event.CreatureDied.object)
    }
  }
  return facts
}

/** The attackers and blockers that the combat after has and the combat before did not. */
function joinedCombat(
  before: CombatView,
  after: CombatView,
): { attackers: ObjectId[]; blockers: ObjectId[] } {
  const attacking = new Set(before.attackers.map((attacker) => attacker.object))
  const blocking = new Set(before.attackers.flatMap((attacker) => attacker.blockers))
  return {
    attackers: after.attackers.map((attacker) => attacker.object).filter((id) => !attacking.has(id)),
    blockers: after.attackers.flatMap((attacker) => attacker.blockers).filter((id) => !blocking.has(id)),
  }
}

const ascending = (ids: Iterable<ObjectId>): ObjectId[] => [...new Set(ids)].sort((a, b) => a - b)

const onBattlefield = (object: ObjectView | undefined): object is ObjectView => object?.zone === 'Battlefield'

const inGraveyard = (object: ObjectView): boolean =>
  typeof object.zone === 'object' && 'Graveyard' in object.zone

/** The next card after `was` in its row before the batch that is still in that row after it, else null. */
function stillStandingAfter(before: Moment, after: Moment, was: ObjectView): ObjectId | null {
  const row = rowOf(was)
  const old = splitBattlefield(before.view, before.objectsById).get(was.controller)?.[row] ?? []
  const now = new Set(splitBattlefield(after.view, after.objectsById).get(was.controller)?.[row] ?? [])
  return old.slice(old.indexOf(was.id) + 1).find((id) => now.has(id)) ?? null
}

/**
 * The effects of one batch, in a fixed order: life totals by seat, then attackers, blockers, damaged permanents,
 * permanents that entered and permanents that died, each by object id. Empty when nothing worth showing happened.
 */
export function deriveEffects(before: Moment, after: Moment, events: readonly EventView[]): Effect[] {
  const { viewer } = after.view
  const facts = factsOf(events)
  const effects: Effect[] = []

  for (const player of after.view.players) {
    const was = before.view.players.find((candidate) => candidate.seat === player.seat)
    if (was === undefined || was.life === player.life) continue
    effects.push({
      kind: player.life < was.life ? 'damaged' : 'healed',
      seat: player.seat,
      amount: Math.abs(player.life - was.life),
    })
  }

  const changed = after.view.objects.filter(
    (object) => before.objectsById[object.id] !== object && !sameJson(before.objectsById[object.id], object),
  )
  const joined = joinedCombat(before.view.combat, after.view.combat)
  const lean = (kind: 'attacked' | 'blocked', ids: Iterable<ObjectId>): void => {
    for (const id of ascending(ids)) {
      const object = after.objectsById[id]
      if (onBattlefield(object)) effects.push({ kind, object: id, own: object.controller === viewer })
    }
  }
  lean('attacked', [...facts.attackers, ...joined.attackers])
  lean('blocked', [...facts.blockers, ...joined.blockers])

  // Damage dealt by an event, else damage newly marked; a player's share is already in the life totals above.
  const hurt = new Map(facts.damage)
  for (const object of changed) {
    const was = before.objectsById[object.id]?.permanent
    const marked = (object.permanent?.damage ?? 0) - (was?.damage ?? 0)
    if (marked > 0 && was && !hurt.has(object.id)) hurt.set(object.id, marked)
  }
  for (const id of ascending(hurt.keys())) {
    if (onBattlefield(after.objectsById[id]))
      effects.push({ kind: 'damaged', object: id, amount: hurt.get(id) ?? 0 })
  }

  for (const object of changed) {
    if (onBattlefield(object) && !onBattlefield(before.objectsById[object.id])) {
      effects.push({ kind: 'entered', object: object.id })
    }
  }

  // A death is a permanent that stood on the battlefield and now lies in a graveyard, or that a creature-died event
  // names (a token never reaches the graveyard the viewer sees). A player who left the game takes their permanents
  // with them without dying, so those name no such event and stay out.
  const dying = new Set<ObjectId>()
  for (const object of changed) {
    if (inGraveyard(object) && onBattlefield(before.objectsById[object.id])) dying.add(object.id)
  }
  for (const id of facts.dead) if (onBattlefield(before.objectsById[id])) dying.add(id)
  for (const id of ascending(dying).slice(0, MAX_GHOSTS)) {
    const was = before.objectsById[id]
    if (was === undefined) continue
    const died: DiedEffect = {
      kind: 'died',
      object: id,
      seat: was.controller,
      row: rowOf(was),
      snapshot: was,
      before: stillStandingAfter(before, after, was),
      damage: hurt.get(id) ?? 0,
    }
    effects.push(died)
  }
  return effects
}

/** One end of an attachment's move: a permanent (with its controller, to stand in for it when it is not drawn) or a player. */
export type HostEnd = { readonly card: ObjectId; readonly seat?: PlayerId } | { readonly player: PlayerId }

/** A permanent that hung on one host before a batch and hangs on another, or on none, after it. */
export interface AttachChange {
  readonly object: ObjectId
  /** Who controls the attachment, to stand in for it when it is not drawn. */
  readonly seat: PlayerId
  /** Its host before the batch; null when it had none. */
  readonly from: HostEnd | null
  /** Its host after the batch; null when it lost it. */
  readonly to: HostEnd | null
}

const endOf = (host: HostRef | null, moment: Moment): HostEnd | null => {
  if (host === null) return null
  if ('player' in host) return host
  const seat = moment.objectsById[host.card]?.controller
  return seat === undefined ? { card: host.card } : { card: host.card, seat }
}

/**
 * The attachments that changed host between two moments, by object id: an Equipment equipped, moved from one creature
 * to another or unattached, an Aura that moved to a new host. Only permanents on the battlefield in both moments count:
 * an Aura that enters attached is told by the log alone. A host that changes controller keeps its attachments and
 * moves none.
 */
export function deriveAttaches(before: Moment, after: Moment): AttachChange[] {
  const changes: AttachChange[] = []
  for (const object of after.view.objects) {
    const was = before.objectsById[object.id]
    if (!onBattlefield(object) || !onBattlefield(was)) continue
    const from = hostOf(was)
    const to = hostOf(object)
    if (sameHost(from, to)) continue
    changes.push({
      object: object.id,
      seat: object.controller,
      from: endOf(from, before),
      to: endOf(to, after),
    })
  }
  return changes.sort((a, b) => a.object - b.object)
}
