// Goal: the dev table's turn change hands the turn to the other side as the server would (a `TurnBegan` event and the next turn's table), follows on from the table it is played on, changes nothing until it is applied, and makes the banner cue of the turn feed.
import { describe, expect, it } from 'vitest'
import { createPlayStore } from '@/play/store'
import type { PlayStore } from '@/play/store'
import { must } from '@/test-utils/batches'
import { deriveFeed } from '../turn/feed'
import { applyBatchToStore } from './applyBatch'
import { fixtureState } from './devGame'
import { buildFixture } from './fixture'
import { turnChangeBatch } from './turnScene'

function table(seats: number, active?: number): PlayStore {
  const store = createPlayStore()
  store.setState(fixtureState(buildFixture(active === undefined ? { seats } : { seats, active })))
  return store
}

describe('the turn change of the dev table', () => {
  it('does nothing without a table', () => {
    expect(turnChangeBatch(createPlayStore().getState())).toBeNull()
  })

  it.each([
    // seats, active seat before, the seat that has the turn after
    [2, 1, 0],
    [4, 2, 0],
    [4, 0, 1],
    [12, 0, 1],
  ])('at %i seats, seat %i passes the turn to seat %i', (seats, active, after) => {
    const store = table(seats, active)
    const before = must(store.getState().view).turn
    const batch = must(turnChangeBatch(store.getState()))
    expect(batch.from_seq).toBe(store.getState().seq)
    expect(batch.events).toEqual([{ Public: { TurnBegan: { number: before.number + 1, active: after } } }])
    expect(batch.table.turn).toMatchObject({ number: before.number + 1, active: after, step: 'Main1' })
    expect(applyBatchToStore(store, batch)).toBe(true)
    expect(must(store.getState().view).turn.active).toBe(after)
  })

  it('leaves the table untouched until the batch is applied', () => {
    const store = table(4)
    const before = structuredClone(store.getState().view)
    turnChangeBatch(store.getState())
    expect(store.getState().view).toEqual(before)
  })

  it('gives the turn banner its cue: the new turn, and whose it is', () => {
    const store = table(4, 1)
    const was = store.getState()
    const batch = must(turnChangeBatch(was))
    expect(applyBatchToStore(store, batch)).toBe(true)
    const now = store.getState()
    const feed = deriveFeed(
      { view: must(was.view), objectsById: was.objectsById },
      { view: must(now.view), objectsById: now.objectsById },
      batch.events,
      (seat) => `Seat ${seat}`,
    )
    expect(feed.banner).toMatchObject({ yours: true, who: 'Your turn' })
  })
})
