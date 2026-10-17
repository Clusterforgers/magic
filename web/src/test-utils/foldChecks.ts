import { expect } from 'vitest'
import { sameJson } from '@/play/view'
import type { EventBatch, ObjectView, StateView } from '@/protocol/generated'

/** The fold written the plain way: the oracle `applyBatch` must agree with. */
export function reference(view: StateView | null, batch: EventBatch): StateView {
  const byId = new Map<number, ObjectView>(
    (view?.objects ?? []).map((object): [number, ObjectView] => [object.id, object]),
  )
  for (const object of batch.objects) byId.set(object.id, object)
  for (const id of batch.gone) byId.delete(id)
  return { ...batch.table, objects: [...byId.values()].sort((a, b) => a.id - b.id) }
}

const PLAYER_PARTS = ['mana', 'hand', 'library', 'graveyard', 'command'] as const
const TABLE_PARTS = ['turn', 'priority', 'battlefield', 'stack', 'exile', 'combat', 'result'] as const

/** What every fold must satisfy: the oracle's result, objects in id order, and identity kept wherever nothing changed. */
export function expectFold(next: StateView, previous: StateView | null, batch: EventBatch): void {
  expect(next).toEqual(reference(previous, batch))
  const ids = next.objects.map((object) => object.id)
  expect(ids).toEqual([...new Set(ids)].sort((a, b) => a - b))
  if (previous === null) return

  const touched = new Set([...batch.objects.map((object) => object.id), ...batch.gone])
  const before = new Map(previous.objects.map((object): [number, ObjectView] => [object.id, object]))
  for (const object of next.objects) {
    if (!touched.has(object.id)) expect(object).toBe(before.get(object.id))
  }
  const sameObjects =
    next.objects.length === previous.objects.length && next.objects.every((o, i) => o === previous.objects[i])
  if (sameObjects) expect(next.objects).toBe(previous.objects)

  next.players.forEach((player, index) => {
    const old = previous.players[index]
    if (old === undefined) return
    if (sameJson(old, player)) expect(player).toBe(old)
    for (const part of PLAYER_PARTS) {
      if (sameJson(old[part], player[part])) expect(player[part]).toBe(old[part])
    }
  })
  if (
    next.players.length === previous.players.length &&
    next.players.every((p, i) => p === previous.players[i])
  ) {
    expect(next.players).toBe(previous.players)
  }
  for (const part of TABLE_PARTS) {
    if (sameJson(previous[part], next[part])) expect(next[part]).toBe(previous[part])
  }
  if (sameJson(previous, next)) expect(next).toBe(previous)
}
