/**
 * `/dev/table?script=layout`: thirty table states of a typical stretch of play (casts, resolves, an attack, tokens
 * that enter and leave, prompt kinds that switch, a hand that grows and shrinks), for counting how often the table's
 * layout moves (`scripts/layout-jumps.mjs`, `layoutJumps.test.ts`).
 */
import type { PendingDecision } from '@/play/store'
import type { ObjectId, ObjectView, PlayerId, StateView } from '@/protocol/generated'
import { rowOf } from '../cardModel'
import type { RowKind } from '../cardModel'
import type { Fixture } from './fixture'
import { DEV_PROMPTS, DEV_TABLES } from './prompts'

/** One change of the table: what the next state does to the one before it. */
export type ScriptOp =
  | { enter: RowKind; seat: 'me' | 'them' | 'third'; count: number; token?: true }
  | { leave: RowKind; seat: 'me' | 'them' | 'third'; count: number; first?: true }
  | { tap: RowKind; seat: 'me' | 'them'; count: number }
  | { untap: 'me' | 'them' }
  | { draw: number }
  | { discard: number }
  | { cast: 'me' | 'them' }
  | { resolve: RowKind | 'spell' }

/** One step of the script: its changes and the prompt the viewer then faces (a `DEV_PROMPTS` name; none waits). */
export interface ScriptStep {
  name: string
  ops: readonly ScriptOp[]
  prompt: string | null
}

/** The script: thirty states from a quiet main phase through a combat and back. */
export const LAYOUT_SCRIPT: readonly ScriptStep[] = [
  { name: 'start', ops: [], prompt: 'priority' },
  { name: 'draw', ops: [{ draw: 1 }], prompt: 'priority-main' },
  { name: 'cast a creature', ops: [{ cast: 'me' }], prompt: 'pay' },
  { name: 'paid', ops: [{ tap: 'lands', seat: 'me', count: 3 }], prompt: 'priority' },
  { name: 'creature enters', ops: [{ resolve: 'creatures' }], prompt: 'priority' },
  {
    name: 'land played',
    ops: [{ enter: 'lands', seat: 'me', count: 1 }, { discard: 1 }],
    prompt: 'priority',
  },
  { name: 'removal aimed', ops: [{ cast: 'me' }], prompt: 'targets' },
  {
    name: 'their creature dies',
    ops: [{ resolve: 'spell' }, { leave: 'creatures', seat: 'them', count: 1 }],
    prompt: 'priority',
  },
  { name: 'they cast', ops: [{ cast: 'them' }], prompt: 'priority' },
  { name: 'their creature enters', ops: [{ resolve: 'creatures' }], prompt: 'priority' },
  {
    name: 'two tokens',
    ops: [{ enter: 'creatures', seat: 'me', count: 2, token: true }],
    prompt: 'priority',
  },
  { name: 'attack', ops: [], prompt: 'attack' },
  { name: 'attackers tap', ops: [{ tap: 'creatures', seat: 'me', count: 3 }], prompt: 'priority' },
  {
    name: 'damage',
    ops: [
      { leave: 'creatures', seat: 'them', count: 1 },
      { leave: 'creatures', seat: 'me', count: 1 },
    ],
    prompt: 'priority',
  },
  { name: 'a token dies', ops: [{ leave: 'creatures', seat: 'me', count: 1 }], prompt: 'priority' },
  { name: 'scry', ops: [], prompt: 'scry' },
  { name: 'draw two', ops: [{ draw: 2 }], prompt: 'priority-main' },
  { name: 'modes', ops: [{ cast: 'me' }], prompt: 'mode' },
  { name: 'pay again', ops: [{ tap: 'lands', seat: 'me', count: 2 }], prompt: 'pay' },
  { name: 'artifact enters', ops: [{ resolve: 'other' }], prompt: 'priority' },
  { name: 'their two enter', ops: [{ enter: 'creatures', seat: 'them', count: 2 }], prompt: 'priority' },
  { name: 'block', ops: [{ tap: 'creatures', seat: 'them', count: 2 }], prompt: 'block' },
  {
    name: 'first creature dies',
    ops: [{ leave: 'creatures', seat: 'me', count: 1, first: true }],
    prompt: 'priority',
  },
  { name: 'yes or no', ops: [], prompt: 'yesno' },
  { name: 'choose cards', ops: [{ enter: 'other', seat: 'third', count: 1 }], prompt: 'cards' },
  { name: 'untap', ops: [{ untap: 'me' }, { untap: 'them' }, { draw: 1 }], prompt: 'priority' },
  { name: 'discard', ops: [], prompt: 'discard' },
  {
    name: 'discarded',
    ops: [{ discard: 2 }, { enter: 'creatures', seat: 'me', count: 3, token: true }],
    prompt: 'priority',
  },
  {
    name: 'their land',
    ops: [
      { enter: 'lands', seat: 'them', count: 1 },
      { leave: 'creatures', seat: 'third', count: 1 },
    ],
    prompt: 'priority',
  },
  { name: 'end step', ops: [{ leave: 'creatures', seat: 'me', count: 2 }], prompt: 'priority-end' },
]

/** A state of the script: the table and the decision the viewer faces. */
export interface ScriptState {
  name: string
  fixture: Fixture
  pending: PendingDecision | null
}

const seatOf = (view: StateView, who: 'me' | 'them' | 'third'): PlayerId => {
  const seats = view.players.length
  const offset = who === 'me' ? 0 : who === 'them' ? 1 : Math.min(2, seats - 1)
  return (view.viewer + offset) % seats
}

/** The battlefield's permanents of `seat` in `lane`, in the battlefield's order. */
const laneOf = (view: StateView, seat: PlayerId, lane: RowKind): ObjectView[] => {
  const byId = new Map(view.objects.map((object) => [object.id, object]))
  return (view.battlefield.cards ?? [])
    .map((id) => byId.get(id))
    .filter(
      (object): object is ObjectView =>
        object !== undefined && object.controller === seat && rowOf(object) === lane,
    )
}

const nextId = (view: StateView): ObjectId => Math.max(0, ...view.objects.map((object) => object.id)) + 1

const withObjects = (view: StateView, change: (object: ObjectView) => ObjectView): StateView => ({
  ...view,
  objects: view.objects.map(change),
})

/** A permanent of `lane` for `seat` like one it has (or any seat has), fresh and untapped. */
function freshPermanent(
  view: StateView,
  seat: PlayerId,
  lane: RowKind,
  id: ObjectId,
  token: boolean,
): ObjectView | null {
  const like =
    laneOf(view, seat, lane)[0] ??
    view.objects.find((object) => object.zone === 'Battlefield' && rowOf(object) === lane)
  if (like === undefined) return null
  return {
    ...like,
    id,
    owner: seat,
    controller: seat,
    zone: 'Battlefield',
    ...(token ? { kind: 'Token' as const, name: `${like.name} token` } : {}),
    permanent: { tapped: false, damage: 0, summoning_sick: true },
  }
}

const enter = (view: StateView, objects: readonly ObjectView[]): StateView => ({
  ...view,
  objects: [...view.objects, ...objects],
  battlefield: {
    count: view.battlefield.count + objects.length,
    cards: [...(view.battlefield.cards ?? []), ...objects.map((object) => object.id)],
  },
})

/** The objects of `ids` gone from the battlefield (or the stack) to their owner's graveyard. */
function toGraveyard(view: StateView, ids: readonly ObjectId[]): StateView {
  const gone = new Set(ids)
  const keep = (cards: ObjectId[] | null): ObjectId[] | null => cards?.filter((id) => !gone.has(id)) ?? null
  const battlefield = keep(view.battlefield.cards)
  const stack = keep(view.stack.cards)
  return {
    ...withObjects(view, (object) =>
      gone.has(object.id) ? { ...object, zone: { Graveyard: object.owner }, permanent: null } : object,
    ),
    battlefield: { count: battlefield?.length ?? 0, cards: battlefield },
    stack: { count: stack?.length ?? 0, cards: stack },
    players: view.players.map((player) => {
      const mine = ids.filter((id) => view.objects.find((object) => object.id === id)?.owner === player.seat)
      if (mine.length === 0) return player
      const cards = [...(player.graveyard.cards ?? []), ...mine]
      return { ...player, graveyard: { count: cards.length, cards } }
    }),
  }
}

const withHand = (view: StateView, cards: ObjectId[]): StateView => ({
  ...view,
  players: view.players.map((player) =>
    player.seat === view.viewer ? { ...player, hand: { count: cards.length, cards } } : player,
  ),
})

/** Applies one change to the view. */
function applyOp(view: StateView, op: ScriptOp): StateView {
  if ('enter' in op) {
    const seat = seatOf(view, op.seat)
    const first = nextId(view)
    const fresh = Array.from({ length: op.count }, (_, index) =>
      freshPermanent(view, seat, op.enter, first + index, op.token === true),
    ).filter((object): object is ObjectView => object !== null)
    return enter(view, fresh)
  }
  if ('leave' in op) {
    const lane = laneOf(view, seatOf(view, op.seat), op.leave)
    const gone = op.first === true ? lane.slice(0, op.count) : lane.slice(-op.count)
    return toGraveyard(
      view,
      gone.map((object) => object.id),
    )
  }
  if ('tap' in op) {
    const ready = laneOf(view, seatOf(view, op.seat), op.tap).filter(
      (object) => object.permanent?.tapped === false,
    )
    const tapped = new Set(ready.slice(0, op.count).map((object) => object.id))
    return withObjects(view, (object) =>
      tapped.has(object.id) && object.permanent !== null
        ? { ...object, permanent: { ...object.permanent, tapped: true } }
        : object,
    )
  }
  if ('untap' in op) {
    const seat = seatOf(view, op.untap)
    return withObjects(view, (object) =>
      object.zone === 'Battlefield' && object.controller === seat && object.permanent !== null
        ? { ...object, permanent: { ...object.permanent, tapped: false, summoning_sick: false } }
        : object,
    )
  }
  const hand = view.players[view.viewer]?.hand.cards ?? []
  if ('draw' in op) {
    const like = view.objects.find((object) => hand.includes(object.id))
    if (like === undefined) return view
    const first = nextId(view)
    const drawn = Array.from({ length: op.draw }, (_, index) => ({ ...like, id: first + index }))
    return withHand({ ...view, objects: [...view.objects, ...drawn] }, [
      ...hand,
      ...drawn.map((object) => object.id),
    ])
  }
  if ('discard' in op) return withHand(view, hand.slice(0, Math.max(0, hand.length - op.discard)))
  if ('cast' in op) {
    const seat = seatOf(view, op.cast)
    const id = nextId(view)
    const spell =
      view.objects.find((object) => object.zone === 'Stack') ??
      view.objects.find((object) => hand.includes(object.id))
    if (spell === undefined) return view
    const cast: ObjectView = { ...spell, id, owner: seat, controller: seat, zone: 'Stack', permanent: null }
    const stack = [...(view.stack.cards ?? []), id]
    const left = op.cast === 'me' ? withHand(view, hand.slice(0, -1)) : view
    return { ...left, objects: [...left.objects, cast], stack: { count: stack.length, cards: stack } }
  }
  // Resolve: the top of the stack leaves it, and a permanent spell enters for its controller.
  const top = view.stack.cards?.at(-1)
  const spell = view.objects.find((object) => object.id === top)
  if (spell === undefined) return view
  const gone = toGraveyard(view, [spell.id])
  if (op.resolve === 'spell') return gone
  const permanent = freshPermanent(gone, spell.controller, op.resolve, nextId(gone), false)
  return permanent === null ? gone : enter(gone, [permanent])
}

/** The start of the script: the table with its stack resolved and no combat, as a main phase has it. */
function quiet(fixture: Fixture): Fixture {
  const cleared = toGraveyard(fixture.view, fixture.view.stack.cards ?? [])
  const view = { ...cleared, combat: { attackers: [], attackers_declared: false, blockers_declared: [] } }
  return { ...fixture, view }
}

/** Every state of `script` over `base`, the changes adding up; a prompt's own moment (`DEV_TABLES`) is not kept. */
export function scriptStates(base: Fixture, script: readonly ScriptStep[] = LAYOUT_SCRIPT): ScriptState[] {
  let fixture = quiet(base)
  return script.map((step) => {
    fixture = { ...fixture, view: step.ops.reduce(applyOp, fixture.view) }
    const prompt = step.prompt === null ? undefined : DEV_PROMPTS[step.prompt]
    const table = step.prompt === null ? undefined : DEV_TABLES[step.prompt]
    const shown = table === undefined ? fixture : table(fixture)
    return { name: step.name, fixture: shown, pending: prompt === undefined ? null : prompt(shown) }
  })
}
