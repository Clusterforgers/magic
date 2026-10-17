// Goal: a screen that reads cards by id from the session store re-renders a card only when a batch changed that card, so a whole game costs about one render per changed object.
import { act, render } from '@testing-library/react'
import { memo } from 'react'
import { describe, expect, it } from 'vitest'
import { useStore } from 'zustand'
import { startSession } from '@/test-utils/harness'
import { lateJoin } from '@/test-utils/transcripts'
import type { PlayStore } from './store'

const NO_CARDS: number[] = []

type Counts = Map<number, number>

const Card = memo(function Card({ store, id, counts }: { store: PlayStore; id: number; counts: Counts }) {
  const object = useStore(store, (state) => state.objectsById[id])
  counts.set(id, (counts.get(id) ?? 0) + 1)
  return <li>{object?.name}</li>
})

function Board({ store, counts }: { store: PlayStore; counts: Counts }) {
  const ids = useStore(store, (state) => state.view?.battlefield.cards ?? NO_CARDS)
  return (
    <ul>
      {ids.map((id) => (
        <Card key={id} store={store} id={id} counts={counts} />
      ))}
    </ul>
  )
}

describe('a board that reads the session store', () => {
  it('redraws only the cards a batch lists, and mounts new cards once', () => {
    const h = startSession()
    h.session.connect()
    h.open()
    h.deliver(...lateJoin.raw.slice(0, 2))
    const counts: Counts = new Map()
    const { container } = render(<Board store={h.session.store} counts={counts} />)
    const onBoard = (): number[] => h.state().view?.battlefield.cards ?? []
    const initial = onBoard().length
    expect(initial).toBeGreaterThan(0)
    expect(container.querySelectorAll('li')).toHaveLength(initial)
    expect([...counts.values()].every((count) => count === 1)).toBe(true)

    let listedTotal = 0
    let untouchedChecks = 0
    for (const message of lateJoin.messages.slice(2)) {
      const before = new Map(counts)
      const stillThere = new Set(onBoard())
      const listed =
        message.type === 'Events' ? new Set(message.objects.map((object) => object.id)) : new Set<number>()
      listedTotal += listed.size
      act(() => {
        h.deliver(message)
      })
      for (const id of onBoard()) {
        if (stillThere.has(id) && !listed.has(id)) {
          expect(counts.get(id), `card ${id} after ${message.type}`).toBe(before.get(id))
          untouchedChecks += 1
        } else {
          expect(counts.get(id) ?? 0).toBeLessThanOrEqual((before.get(id) ?? 0) + 1)
        }
      }
    }
    const renders = [...counts.values()].reduce((sum, count) => sum + count, 0)
    expect(untouchedChecks).toBeGreaterThan(100)
    expect(renders).toBeLessThanOrEqual(initial + listedTotal)
    expect(container.querySelectorAll('li')).toHaveLength(onBoard().length)
  })

  it('redraws nothing when a Snapshot after a reconnect describes the board it already shows', () => {
    const h = startSession()
    h.session.connect()
    h.open()
    h.deliver(...lateJoin.raw.slice(0, 2))
    const counts: Counts = new Map()
    render(<Board store={h.session.store} counts={counts} />)
    act(() => {
      // Everything but the final GameOver, which would make the lost connection final.
      h.deliver(...lateJoin.raw.slice(2, -1))
    })
    const { view, seq, pending } = h.state()
    expect(pending).not.toBeNull()
    const before = new Map(counts)
    act(() => {
      h.net.last.fail()
      h.timers.advance(250)
      h.open()
      // The server's rejoin answer: Welcome, then the snapshot of the moment the client already has.
      h.deliver(lateJoin.raw[0], { type: 'Snapshot', seq, state: view })
    })
    expect(h.net.sockets).toHaveLength(2)
    expect(h.state()).toMatchObject({ status: 'open', pending: null, seq })
    expect(counts).toEqual(before)
    expect(h.state().view).toBe(view)
  })
})
