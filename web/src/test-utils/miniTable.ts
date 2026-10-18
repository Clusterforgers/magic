/**
 * A table small enough to name every object in a test, and a way to turn it into the next one the way the server
 * does: a batch with a table, the objects that changed and the ones that are gone, folded by the real `applyBatch`.
 */
import type { Fixture } from '@/features/table/dev/fixture'
import type { Moment } from '@/features/table/fx/derive'
import { applyBatch, indexById } from '@/play/view'
import type {
  Event,
  EventBatch,
  EventView,
  ObjectId,
  ObjectView,
  PlayerId,
  PlayerView,
  StateView,
  ZoneView,
} from '@/protocol/generated'
import { clone, must, tableOf } from './batches'
import { card, forest } from './cards'

/** The viewer's seat and the opponent's. */
export const ME = 0
export const FOE = 1

/** The viewer's creatures, a 2/2, a 3/3 and a 1/4, and their Forest. */
export const MY_BEAR = 1
export const MY_GIANT = 2
export const MY_WALL = 3
export const MY_FOREST = 4
/** The opponent's creatures, a 2/2, a 4/4 and a 1/1, their Forest and their artifact. */
export const FOE_BEAR = 11
export const FOE_GOLEM = 12
export const FOE_IMP = 13
export const FOE_FOREST = 14
export const FOE_IDOL = 15
/** A card in the viewer's hand, and a creature spell on the stack that the opponent cast. */
export const MY_HAND_CARD = 5
export const FOE_SPELL = 30

const zone = (cards: ObjectId[]): ZoneView => ({ count: cards.length, cards })

const seat = (id: PlayerId, hand: ObjectId[] | null): PlayerView => ({
  seat: id,
  life: 20,
  lost: null,
  mana: { counts: [0, 0, 0, 0, 0, 0] },
  mulligans: 0,
  mulligan: 'Kept',
  hand: hand === null ? { count: 4, cards: null } : zone(hand),
  library: { count: 30, cards: null },
  graveyard: zone([]),
  command: zone([]),
})

const creature = (
  id: number,
  owner: PlayerId,
  name: string,
  power: number,
  toughness: number,
  over: Partial<ObjectView> = {},
): ObjectView => card({ id, owner, controller: owner, name, power, toughness, ...over })

/**
 * Seat 0 (the viewer) and seat 1, each with three creatures, a Forest and 20 life; the opponent also has an artifact
 * and a creature spell on the stack. It is the opponent's turn, in the main phase, with no combat.
 */
export function miniView(): StateView {
  const objects: ObjectView[] = [
    creature(MY_BEAR, ME, 'Ash Bear', 2, 2),
    creature(MY_GIANT, ME, 'Stone Giant', 3, 3, { colors: ['Red'] }),
    creature(MY_WALL, ME, 'Wall of Reeds', 1, 4, { colors: ['Blue'] }),
    forest({ id: MY_FOREST, owner: ME, controller: ME }),
    card({
      id: MY_HAND_CARD,
      owner: ME,
      controller: ME,
      zone: { Hand: ME },
      name: 'Rampant Growth',
      permanent: null,
    }),
    creature(FOE_BEAR, FOE, 'Cave Bear', 2, 2),
    creature(FOE_GOLEM, FOE, 'Iron Golem', 4, 4, { colors: [], types: ['Artifact', 'Creature'] }),
    creature(FOE_IMP, FOE, 'Pit Imp', 1, 1, { colors: ['Black'] }),
    forest({ id: FOE_FOREST, owner: FOE, controller: FOE }),
    card({
      id: FOE_IDOL,
      owner: FOE,
      controller: FOE,
      name: 'Gold Idol',
      colors: [],
      types: ['Artifact'],
      subtypes: [],
      power: null,
      toughness: null,
    }),
    creature(FOE_SPELL, FOE, 'Hill Giant', 3, 3, { zone: 'Stack', permanent: null, colors: ['Red'] }),
  ]
  return {
    objects,
    viewer: ME,
    format: 'Normal',
    stage: 'Turns',
    first_player: ME,
    turn: { number: 3, active: FOE, step: 'Main1', lands_played: 0 },
    priority: { holder: ME, passed: [] },
    players: [seat(ME, [MY_HAND_CARD]), seat(FOE, null)],
    battlefield: zone([
      MY_BEAR,
      MY_GIANT,
      MY_WALL,
      MY_FOREST,
      FOE_BEAR,
      FOE_GOLEM,
      FOE_IMP,
      FOE_FOREST,
      FOE_IDOL,
    ]),
    stack: zone([FOE_SPELL]),
    exile: zone([]),
    combat: { attackers: [], attackers_declared: false, blockers_declared: [] },
    result: null,
  }
}

/** `miniView()` with names, ready for the table harness. */
export function miniFixture(): Fixture {
  return { view: miniView(), log: [], chat: [], names: ['Alice', 'Bo'] }
}

/** A view with its objects indexed by id, as the play store keeps it. */
export function momentOf(view: StateView): Moment {
  return { view, objectsById: indexById(view.objects) }
}

/** The changes a test makes to the next table; every one is listed in the batch it builds. */
export interface Draft {
  /** The next view, which a test may also change by hand (life, combat, the turn). */
  readonly view: StateView
  /** Changes object `id` and lists it as changed. */
  edit(id: ObjectId, change: (object: ObjectView) => void): void
  /** Adds an object to the view and lists it. */
  add(object: ObjectView): void
  /** An object the viewer can no longer see: out of the view and the zones, listed as gone. */
  leave(id: ObjectId): void
  /** A battlefield permanent goes to its owner's graveyard. */
  die(id: ObjectId): void
  /** An object from the stack or a hand comes onto the battlefield under its owner's control. */
  enter(id: ObjectId): void
  /** Sets the life of a seat. */
  life(seat: PlayerId, life: number): void
}

const without = (zoneView: ZoneView, id: ObjectId): ZoneView =>
  zoneView.cards === null
    ? { count: zoneView.count, cards: null }
    : zone(zoneView.cards.filter((candidate) => candidate !== id))

/** The batch that turns `view` into the table `change` makes of a copy of it. */
export function batchFrom(
  view: StateView,
  change: (draft: Draft) => void,
  events: readonly EventView[] = [],
  fromSeq = 1,
): EventBatch {
  const next = clone(view)
  const touched = new Set<ObjectId>()
  const gone = new Set<ObjectId>()
  const find = (id: ObjectId): ObjectView =>
    must(
      next.objects.find((object) => object.id === id),
      `object ${id}`,
    )
  const draft: Draft = {
    view: next,
    edit(id, edit) {
      edit(find(id))
      touched.add(id)
    },
    add(object) {
      next.objects = [...next.objects, object].sort((a, b) => a.id - b.id)
      touched.add(object.id)
    },
    leave(id) {
      next.objects = next.objects.filter((object) => object.id !== id)
      next.battlefield = without(next.battlefield, id)
      gone.add(id)
    },
    die(id) {
      const owner = find(id).owner
      draft.edit(id, (object) => {
        object.zone = { Graveyard: owner }
        object.permanent = null
      })
      next.battlefield = without(next.battlefield, id)
      const player = must(next.players[owner])
      player.graveyard = zone([...(player.graveyard.cards ?? []), id])
    },
    enter(id) {
      draft.edit(id, (object) => {
        object.zone = 'Battlefield'
        object.permanent = { tapped: false, damage: 0, summoning_sick: true }
      })
      next.stack = without(next.stack, id)
      next.battlefield = zone([...(next.battlefield.cards ?? []), id])
    },
    life(id, life) {
      must(next.players[id]).life = life
    },
  }
  change(draft)
  return {
    from_seq: fromSeq,
    events: [...events],
    table: tableOf(next),
    objects: [...touched].filter((id) => !gone.has(id)).map(find),
    gone: [...gone],
  }
}

/** The moment that follows `before` once `change` has happened, folded by the real client fold. */
export function afterChange(
  before: Moment,
  change: (draft: Draft) => void,
  events: readonly EventView[] = [],
): Moment {
  const view = applyBatch(before.view, batchFrom(before.view, change, events))
  return momentOf(view)
}

/** Public events as a batch carries them. */
export function pub(...events: Event[]): EventView[] {
  return events.map((event) => ({ Public: event }))
}
