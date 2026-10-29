/**
 * Selectors for the table. A component reads exactly what it draws: a card its own object and combat role, a row the
 * ids of its cards. Those that return a new array are meant for `useShallow`, which keeps the old one while the
 * items are the same, so a batch that changes one card redraws that card and nothing around it.
 */
import type { PlayState } from '@/play/store'
import type { ObjectId, ObjectView, PlayerId, PlayerView, StateView } from '@/protocol/generated'
import { combatRoleOf, rowOf } from './cardModel'
import type { CombatRole, RowKind } from './cardModel'
import { defaultFocus } from './layout'

/** One shared empty list, so "nothing" is the same value every time. */
export const NO_IDS: readonly ObjectId[] = []

/** A seat's permanents, split into rows, each in battlefield order. */
export type BattlefieldRows = Record<RowKind, ObjectId[]>

type Objects = Readonly<Record<ObjectId, ObjectView>>

// The shared battlefield is split once per view, however many seats ask.
const splits = new WeakMap<StateView, ReadonlyMap<PlayerId, BattlefieldRows>>()

/**
 * The attachments that travel with a host: an Aura, Equipment or Fortification sits in its host's row, right behind
 * it, while the host is on the battlefield under the same controller. One attached to another seat's permanent or
 * to a player stays in its own controller's row, where its "On <host>" chip names the host.
 */
function escorts(ids: readonly ObjectId[], objects: Objects): ReadonlyMap<ObjectId, ObjectId[]> {
  const onBattlefield = new Set(ids)
  const byHost = new Map<ObjectId, ObjectId[]>()
  for (const id of ids) {
    const object = objects[id]
    const host = object?.attached_to === undefined ? undefined : objects[object.attached_to]
    if (object === undefined || host === undefined || host.id === id) continue
    if (host.controller !== object.controller || !onBattlefield.has(host.id)) continue
    byHost.set(host.id, [...(byHost.get(host.id) ?? []), id])
  }
  return byHost
}

/** The battlefield by controller and row. Cached per view; a view is never changed, only replaced. */
export function splitBattlefield(view: StateView, objects: Objects): ReadonlyMap<PlayerId, BattlefieldRows> {
  const cached = splits.get(view)
  if (cached !== undefined) return cached
  const bySeat = new Map<PlayerId, BattlefieldRows>()
  const ids = view.battlefield.cards ?? NO_IDS
  const byHost = escorts(ids, objects)
  const escorted = new Set([...byHost.values()].flat())
  const placed = new Set<ObjectId>()
  const place = (id: ObjectId, rows: BattlefieldRows, row: RowKind): void => {
    if (placed.has(id)) return
    placed.add(id)
    rows[row].push(id)
    for (const attachment of byHost.get(id) ?? NO_IDS) place(attachment, rows, row)
  }
  // An attachment is placed with its host; one whose chain of hosts is a loop is placed on its own afterwards.
  for (const pass of [false, true]) {
    for (const id of ids) {
      const object = objects[id]
      if (object === undefined || (!pass && escorted.has(id))) continue
      let rows = bySeat.get(object.controller)
      if (rows === undefined) {
        rows = { creatures: [], other: [], lands: [] }
        bySeat.set(object.controller, rows)
      }
      place(id, rows, rowOf(object))
    }
  }
  splits.set(view, bySeat)
  return bySeat
}

const ROW_KINDS: readonly RowKind[] = ['creatures', 'other', 'lands']

/** The row of its controller's battlefield that holds a permanent: its own, or its host's while it travels with one. */
export function rowHolding(view: StateView, objects: Objects, object: ObjectView): RowKind {
  const rows = splitBattlefield(view, objects).get(object.controller)
  const held = rows === undefined ? undefined : ROW_KINDS.find((row) => rows[row].includes(object.id))
  return held ?? rowOf(object)
}

/** The ids in one row of a seat's battlefield; wrap in `useShallow`. */
export function battlefieldRow(state: PlayState, seat: PlayerId, row: RowKind): readonly ObjectId[] {
  if (state.view === null) return NO_IDS
  return splitBattlefield(state.view, state.objectsById).get(seat)?.[row] ?? NO_IDS
}

/** How many permanents of each row a seat has, for a tile. */
export function battlefieldCounts(state: PlayState, seat: PlayerId): Record<RowKind, number> {
  const rows = state.view === null ? undefined : splitBattlefield(state.view, state.objectsById).get(seat)
  return {
    creatures: rows?.creatures.length ?? 0,
    other: rows?.other.length ?? 0,
    lands: rows?.lands.length ?? 0,
  }
}

/** Whether a seat controls nothing on the battlefield. */
export function isBattlefieldEmpty(state: PlayState, seat: PlayerId): boolean {
  const { creatures, other, lands } = battlefieldCounts(state, seat)
  return creatures + other + lands === 0
}

/** A seat's view of its own player, or undefined for a seat that is not at the table. */
export function playerOf(state: PlayState, seat: PlayerId): PlayerView | undefined {
  return state.view?.players.find((player) => player.seat === seat)
}

/** The role of one object in the current combat; a string, so it needs no `useShallow`. */
export function combatRoleIn(state: PlayState, id: ObjectId): CombatRole | null {
  return state.view === null ? null : combatRoleOf(state.view.combat, id)
}

/** The objects in exile that `seat` owns (exile is one shared zone); wrap in `useShallow`. */
export function exileOf(state: PlayState, seat: PlayerId): ObjectId[] {
  const cards = state.view?.exile.cards ?? NO_IDS
  return cards.filter((id) => state.objectsById[id]?.owner === seat)
}

/** The stack, top first; wrap in `useShallow`. */
export function stackTopFirst(state: PlayState): ObjectId[] {
  return [...(state.view?.stack.cards ?? NO_IDS)].reverse()
}

/** The number of seats at the table. */
export function seatCountOf(state: PlayState): number {
  return state.view?.players.length ?? state.seatCount ?? 0
}

/** The opponent to show large when nobody chose one; a number, so it needs no `useShallow`. */
export function defaultFocusOf(state: PlayState): PlayerId | null {
  const { view, seat } = state
  if (view === null || seat === null) return null
  const eliminated = view.players.filter((player) => player.lost !== null).map((player) => player.seat)
  return defaultFocus(view.players.length, seat, {
    active: view.turn.active,
    holder: view.priority?.holder ?? null,
    eliminated,
  })
}

/** Whether turns are being played: the stage is Turns and the game has no result; a boolean, so it needs no `useShallow`. */
export function turnsRunning(state: PlayState): boolean {
  const { view } = state
  return view !== null && view.stage === 'Turns' && view.result === null
}

/** Whether `seat` has the turn of a running game. Every seat sees the same, so a cue made from it leaks nothing. */
export function isActiveSeat(state: PlayState, seat: PlayerId): boolean {
  return turnsRunning(state) && state.view?.turn.active === seat
}

/** Whether `seat` holds priority; public, like the turn. */
export function holdsPriority(state: PlayState, seat: PlayerId): boolean {
  return state.view?.priority?.holder === seat
}

/** Whether it is this seat's own turn in a running game; a boolean, so it needs no `useShallow`. */
export function isYourTurn(state: PlayState): boolean {
  const { seat } = state
  return seat !== null && isActiveSeat(state, seat)
}

/** Whether this seat may still concede: it is in the game, which has not ended. */
export function canConcede(state: PlayState): boolean {
  const { view, seat } = state
  if (seat === null || view === null || state.phase !== 'playing') return false
  if (view.stage === 'Finished' || view.result !== null) return false
  const player = playerOf(state, seat)
  return player !== undefined && player.lost === null
}

/** Whether stops and Full Control mean anything: the seat is still in a running game. */
export function canSetStops(state: PlayState): boolean {
  return canConcede(state)
}
