/**
 * Moves on the fixture table for looking at the table effects (`?fx=` and the buttons of `DevFxControls`). Each builds
 * the batch the server would send for it, from the table as it stands, and plays it through the same fold and the
 * same effects layer as a real game, so pressing one again picks the next creatures.
 */
import type { PlayState, PlayStore } from '@/play/store'
import { sameJson } from '@/play/view'
import type {
  AttackTarget,
  Event,
  EventBatch,
  ObjectId,
  ObjectView,
  PlayerId,
  StateView,
  TableView,
} from '@/protocol/generated'
import type { DevFx } from '../devSearch'
import { applyBatchToStore } from './applyBatch'

/** The table a move changes, and the means to change it. */
interface Scene {
  next: StateView
  viewer: PlayerId
  foe: PlayerId
  /** Changes an object and lists it in the batch. */
  edit(id: ObjectId, change: (object: ObjectView) => void): void
  /** Adds an object and lists it in the batch. */
  add(object: ObjectView): void
}

function tableOf(view: StateView): TableView {
  return {
    viewer: view.viewer,
    format: view.format,
    stage: view.stage,
    first_player: view.first_player,
    turn: view.turn,
    priority: view.priority,
    players: view.players,
    battlefield: view.battlefield,
    stack: view.stack,
    exile: view.exile,
    combat: view.combat,
    combat_forecast: view.combat_forecast,
    result: view.result,
  }
}

/** A seat's creatures on the battlefield, in battlefield order. */
function creaturesOf(view: StateView, seat: PlayerId): ObjectView[] {
  const byId = new Map(view.objects.map((object) => [object.id, object]))
  return (view.battlefield.cards ?? []).flatMap((id) => {
    const object = byId.get(id)
    return object !== undefined && object.controller === seat && object.types.includes('Creature')
      ? [object]
      : []
  })
}

const toughnessLeft = (object: ObjectView): number =>
  (object.toughness ?? 0) - (object.permanent?.damage ?? 0)

/** Every creature that attacks or blocks in the combat now. */
const inCombat = (view: StateView): Set<ObjectId> =>
  new Set(view.combat.attackers.flatMap((attacker) => [attacker.object, ...attacker.blockers]))

function tap(scene: Scene, objects: readonly ObjectView[]): void {
  for (const object of objects) {
    scene.edit(object.id, (target) => {
      if (target.permanent !== null) target.permanent.tapped = true
    })
  }
}

/** A creature of the other side to take the blame for damage. */
const culprit = (scene: Scene, victim: ObjectView): ObjectId =>
  creaturesOf(scene.next, victim.controller === scene.viewer ? scene.foe : scene.viewer)[0]?.id ?? victim.id

const MOVES: Record<DevFx, (scene: Scene) => Event[]> = {
  // Two of the viewer's creatures that are not in the combat attack the opponent.
  attack(scene) {
    const { next, viewer, foe } = scene
    const busy = inCombat(next)
    const chosen = creaturesOf(next, viewer)
      .filter((object) => !busy.has(object.id))
      .slice(0, 2)
    if (chosen.length === 0) return []
    next.combat = {
      attackers: chosen.map((object) => ({
        object: object.id,
        target: { Player: foe },
        blocked: false,
        blockers: [],
      })),
      attackers_declared: true,
      blockers_declared: [],
    }
    tap(scene, chosen)
    return [
      {
        AttackersDeclared: {
          player: viewer,
          attackers: chosen.map((object): [ObjectId, AttackTarget] => [object.id, { Player: foe }]),
        },
      },
    ]
  },

  // An opponent's creature attacks the viewer and two of the viewer's creatures block it.
  block(scene) {
    const { next, viewer, foe } = scene
    const busy = inCombat(next)
    const [attacker] = creaturesOf(next, foe).filter((object) => !busy.has(object.id))
    const blockers = creaturesOf(next, viewer)
      .filter((object) => !busy.has(object.id))
      .slice(0, 2)
    if (attacker === undefined || blockers.length === 0) return []
    next.combat = {
      attackers: [
        {
          object: attacker.id,
          target: { Player: viewer },
          blocked: true,
          blockers: blockers.map((o) => o.id),
        },
      ],
      attackers_declared: true,
      blockers_declared: [viewer],
    }
    tap(scene, [attacker])
    return [
      { AttackersDeclared: { player: foe, attackers: [[attacker.id, { Player: viewer }]] } },
      {
        BlockersDeclared: {
          player: viewer,
          blocks: blockers.map((object): [ObjectId, ObjectId] => [object.id, attacker.id]),
        },
      },
    ]
  },

  // Two of the opponent's creatures and one of the viewer's take damage and live.
  damage(scene) {
    const { next, viewer, foe, edit } = scene
    const survivors = (seat: PlayerId, count: number): ObjectView[] =>
      creaturesOf(next, seat)
        .filter((object) => toughnessLeft(object) >= 2)
        .slice(0, count)
    const events: Event[] = []
    for (const victim of [...survivors(foe, 2), ...survivors(viewer, 1)]) {
      const amount = Math.min(2, toughnessLeft(victim) - 1)
      edit(victim.id, (object) => {
        if (object.permanent !== null) object.permanent.damage += amount
      })
      events.push({ DamageDealt: { source: culprit(scene, victim), target: { Object: victim.id }, amount } })
    }
    return events
  },

  // One creature of each side, outside the combat, takes lethal damage and goes to its graveyard.
  death(scene) {
    const { next, viewer, foe, edit } = scene
    const busy = inCombat(next)
    const events: Event[] = []
    const victims = [foe, viewer].flatMap(
      (seat) => creaturesOf(next, seat).find((o) => !busy.has(o.id)) ?? [],
    )
    for (const victim of victims) {
      const source = culprit(scene, victim)
      events.push(
        {
          DamageDealt: { source, target: { Object: victim.id }, amount: Math.max(1, toughnessLeft(victim)) },
        },
        { CreatureDied: { object: victim.id, cause: 'LethalDamage' } },
      )
      edit(victim.id, (object) => {
        object.zone = { Graveyard: object.owner }
        object.permanent = null
      })
      next.battlefield = {
        count: next.battlefield.count - 1,
        cards: (next.battlefield.cards ?? []).filter((id) => id !== victim.id),
      }
      const owner = next.players[victim.owner]
      if (owner !== undefined) {
        owner.graveyard = {
          count: owner.graveyard.count + 1,
          cards: [...(owner.graveyard.cards ?? []), victim.id],
        }
      }
    }
    return events
  },

  // The viewer gains 4 life and the opponent loses 5.
  life({ next, viewer, foe }) {
    const mine = next.players[viewer]
    const theirs = next.players[foe]
    if (mine === undefined || theirs === undefined) return []
    mine.life += 4
    theirs.life -= 5
    const [source] = creaturesOf(next, viewer)
    return [
      source === undefined
        ? { PriorityPassed: { player: viewer } }
        : { DamageDealt: { source: source.id, target: { Player: foe }, amount: 5 } },
    ]
  },

  // A copy of the first creature of each side comes onto the battlefield.
  enter(scene) {
    const { next, viewer, foe, add } = scene
    const events: Event[] = []
    let id = Math.max(0, ...next.objects.map((object) => object.id))
    for (const seat of [viewer, foe]) {
      const [template] = creaturesOf(next, seat)
      if (template === undefined) continue
      id += 1
      add({ ...structuredClone(template), id, permanent: { tapped: false, damage: 0, summoning_sick: true } })
      next.battlefield = { count: next.battlefield.count + 1, cards: [...(next.battlefield.cards ?? []), id] }
      events.push({ SpellResolved: { object: id } })
    }
    return events
  },
}

/** The batch for `kind` from the table in `state`, or null when there is no table or the move has nothing to do. */
export function devBatch(kind: DevFx, state: PlayState): EventBatch | null {
  const { view, seq } = state
  if (view === null || seq === null) return null
  const next = structuredClone(view)
  const byId = new Map(next.objects.map((object) => [object.id, object]))
  const original = new Map(view.objects.map((object) => [object.id, object]))
  const touched = new Set<ObjectId>()
  const scene: Scene = {
    next,
    viewer: next.viewer,
    foe: next.turn.active === next.viewer ? (next.viewer + 1) % next.players.length : next.turn.active,
    edit(id, change) {
      const object = byId.get(id)
      if (object === undefined) return
      change(object)
      touched.add(id)
    },
    add(object) {
      next.objects.push(object)
      byId.set(object.id, object)
      touched.add(object.id)
    },
  }
  const events = MOVES[kind](scene)
  if (events.length === 0) return null
  next.objects.sort((a, b) => a.id - b.id)
  return {
    from_seq: seq,
    events: events.map((event) => ({ Public: event })),
    table: tableOf(next),
    objects: [...touched].flatMap((id) => {
      const object = byId.get(id)
      return object === undefined || sameJson(original.get(id), object) ? [] : [object]
    }),
    gone: [],
  }
}

/** Plays `kind` on the table in `store` as a batch; false when it had nothing to do. */
export function playDevFx(store: PlayStore, kind: DevFx): boolean {
  const batch = devBatch(kind, store.getState())
  return batch !== null && applyBatchToStore(store, batch)
}
