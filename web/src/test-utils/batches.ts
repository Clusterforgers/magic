import type { EventBatch, ObjectView, StateView, TableView } from '@/protocol/generated'

/** The table part of a view: everything but the objects. */
export function tableOf(view: StateView): TableView {
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
    result: view.result,
  }
}

/** The value itself, or a failed test when it is missing; saves a null check on data the test knows is there. */
export function must<T>(value: T | null | undefined, what = 'value'): T {
  if (value === null || value === undefined) throw new Error(`the test expected a ${what}`)
  return value
}

/** A deep copy, as if the value had just been parsed from a message. */
export function clone<T>(value: T): T {
  return structuredClone(value)
}

/** Freezes `value` and everything inside it, so any later write throws. */
export function deepFreeze<T>(value: T): T {
  if (typeof value === 'object' && value !== null) {
    Object.freeze(value)
    const children: unknown[] = Object.values(value)
    for (const child of children) deepFreeze(child)
  }
  return value
}

/** A copy of a permanent with its tapped flag set. */
export function withTapped(object: ObjectView, tapped: boolean): ObjectView {
  const copy = clone(object)
  must(copy.permanent, 'permanent').tapped = tapped
  return copy
}

/** A copy of `object` under another id (and name): a stand-in for a card that entered the view. */
export function relabel(object: ObjectView, id: number, name = object.name): ObjectView {
  return { ...clone(object), id, name }
}

/** A batch that leaves `view` as it is unless `over` says otherwise; its table is a fresh copy, like a parsed one. */
export function batchOf(view: StateView, over: Partial<EventBatch> = {}): EventBatch {
  return { from_seq: 1, events: [], table: clone(tableOf(view)), objects: [], gone: [], ...over }
}
