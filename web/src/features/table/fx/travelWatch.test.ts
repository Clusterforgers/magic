// Goal: the travel layer is started by the batches the session applies and measures a move's source inside the store's subscriber, before the table draws the batch, so a card that has left the hand is still measured where it stood, and its destination on the next frame; a snapshot or a batch without moves starts nothing, and the order of the other subscribers changes nothing.
import { beforeEach, describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import { place, travelFrames } from '@/test-utils/fxDom'
import { startSession } from '@/test-utils/harness'
import { batchFrom, FOE, ME, miniView, MY_HAND_CARD, pub } from '@/test-utils/miniTable'
import type { Draft } from '@/test-utils/miniTable'
import { watchBatches } from './batch'
import { createFx } from './store'

const HAND = { x: 120, y: 760, width: 100, height: 140 }
const CHIP = { x: 440, y: 300, width: 120, height: 32 }

const cast = (draft: Draft): void =>
  draft.edit(MY_HAND_CARD, (object) => {
    object.zone = 'Stack'
    object.permanent = null
  })
const CAST = pub({ SpellCast: { player: ME, object: MY_HAND_CARD } })

let page: HTMLElement
beforeEach(() => {
  page = document.createElement('main')
  document.body.replaceChildren(page)
})

function watched() {
  const h = startSession()
  h.join(ME, 2)
  h.deliver({ type: 'Snapshot', seq: 4, state: miniView() })
  const frames = travelFrames()
  const fx = createFx({ mode: () => 'full', travel: { root: page, frame: frames.scheduler } })
  const hand = place(page, { 'data-object-id': String(MY_HAND_CARD) }, HAND, 'button')
  const stack = place(page, { 'data-fx-anchor': 'stack' }, { x: 0, y: 280, width: 900, height: 60 })
  // The part of React that draws a batch: it runs once the subscribers have, in a microtask.
  h.session.store.subscribe((next, prev) => {
    if (next.view === prev.view) return
    queueMicrotask(() => {
      hand.remove()
      place(stack, { 'data-object-id': String(MY_HAND_CARD) }, CHIP, 'button')
    })
  })
  const stop = watchBatches(h.session.store, fx)
  const send = (change: (draft: Draft) => void, events = CAST): void =>
    h.deliver({
      type: 'Events',
      ...batchFrom(must(h.state().view), change, events, must(h.state().seq)),
    })
  return { h, fx, frames, send, stop }
}

describe('a batch with a cast', () => {
  it('has its source measured before the table draws it and its destination on the next frame', async () => {
    const { fx, frames, send } = watched()
    send(cast)
    // The table has not drawn the batch yet: the card is still in the hand and no chip stands on the stack.
    expect(page.querySelector(`button[data-object-id="${MY_HAND_CARD}"]`)).not.toBeNull()
    expect(page.querySelector('[data-fx-anchor="stack"] [data-object-id]')).toBeNull()
    expect(frames.waiting()).toBe(1)
    await Promise.resolve()
    expect(page.querySelector('[data-fx-anchor="stack"] [data-object-id]')).not.toBeNull()
    expect(page.querySelectorAll(`[data-object-id="${MY_HAND_CARD}"]`)).toHaveLength(1)
    frames.flush()
    const [flight] = fx.travel.store.getState().flights
    expect(flight?.move.kind).toBe('cast')
    expect(flight?.from).toEqual({ box: HAND, card: true })
    expect(flight?.to).toEqual({ box: CHIP, card: false })
  })

  it('is not flown by a batch that casts nothing, or by a snapshot', () => {
    const { h, fx, frames, send } = watched()
    send((draft) => draft.life(FOE, 15), pub({ PriorityPassed: { player: ME } }))
    h.deliver({ type: 'Snapshot', seq: 20, state: miniView() })
    expect(frames.asked()).toBe(0)
    expect(fx.travel.store.getState().flights).toEqual([])
  })

  it('stops with the watcher', () => {
    const { frames, send, stop } = watched()
    stop()
    send(cast)
    expect(frames.asked()).toBe(0)
  })
})
