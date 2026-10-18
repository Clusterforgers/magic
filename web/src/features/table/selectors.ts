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

/** The battlefield by controller and row. Cached per view; a view is never changed, only replaced. */
export function splitBattlefield(view: StateView, objects: Objects): ReadonlyMap<PlayerId, BattlefieldRows> {
  const cached = splits.get(view)
  if (cached !== undefined) return cached
  const bySeat = new Map<PlayerId, BattlefieldRows>()
  for (const id of view.battlefield.cards ?? NO_IDS) {
    const object = objects[id]
    if (object === undefined) continue
    let rows = bySeat.get(object.controller)
    if (rows === undefined) {
      rows = { creatures: [], other: [], lands: [] }
      bySeat.set(object.controller, rows)
    }
    rows[rowOf(object)].push(id)
  }
  splits.set(view, bySeat)
  return bySeat
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
