// Goal: the arrows of attachments that moved start for each change that has a new host (and for no detach), end after exactly one second stretched by the slow-down, are capped per batch, play in reduced motion and not while the layer is off, and stop cleanly when cleared.
import { describe, expect, it } from 'vitest'
import { FakeTimers } from '@/test-utils/fakes'
import { ARROW_MS, createArrows, MAX_ARROWS } from './arrows'
import type { AttachChange } from './derive'

const move = (object: number, to: number | null = 9): AttachChange => ({
  object,
  seat: 0,
  from: { card: 1, seat: 0 },
  to: to === null ? null : { card: to, seat: 0 },
})

function setup(enabled = true, slow = 1) {
  const timers = new FakeTimers()
  const arrows = createArrows({ timers, enabled: () => enabled, slow })
  return { timers, arrows, shown: () => arrows.store.getState() }
}

describe('the arrows of attachments that moved', () => {
  it('draws one arrow for a change with a new host and none for a detach', () => {
    const { arrows, shown } = setup()
    arrows.play([move(5), move(6, null)])
    expect(shown().map((arrow) => arrow.change.object)).toEqual([5])
  })

  it('ends an arrow after one second and not before', () => {
    const { arrows, timers, shown } = setup()
    arrows.play([move(5)])
    timers.advance(ARROW_MS - 1)
    expect(shown()).toHaveLength(1)
    timers.advance(1)
    expect(shown()).toHaveLength(0)
  })

  it('stretches the time by the slow-down', () => {
    const { arrows, timers, shown } = setup(true, 3)
    arrows.play([move(5)])
    timers.advance(ARROW_MS * 3 - 1)
    expect(shown()).toHaveLength(1)
    timers.advance(1)
    expect(shown()).toHaveLength(0)
  })

  it('ends a batch on its own and leaves a later one standing', () => {
    const { arrows, timers, shown } = setup()
    arrows.play([move(5)])
    timers.advance(400)
    arrows.play([move(6)])
    timers.advance(ARROW_MS - 400)
    expect(shown().map((arrow) => arrow.change.object)).toEqual([6])
    timers.advance(400)
    expect(shown()).toHaveLength(0)
  })

  it('gives every arrow a token of its own, so a repeat restarts', () => {
    const { arrows, shown } = setup()
    arrows.play([move(5)])
    arrows.play([move(5)])
    expect(new Set(shown().map((arrow) => arrow.token)).size).toBe(2)
  })

  it('caps the arrows of one batch', () => {
    const { arrows, shown } = setup()
    arrows.play(Array.from({ length: MAX_ARROWS + 3 }, (_, at) => move(at)))
    expect(shown()).toHaveLength(MAX_ARROWS)
  })

  it('plays nothing while the layer is off', () => {
    const { arrows, shown, timers } = setup(false)
    arrows.play([move(5)])
    expect(shown()).toHaveLength(0)
    expect(timers.pending()).toEqual([])
  })

  it('clears every arrow and cancels the timers', () => {
    const { arrows, shown, timers } = setup()
    arrows.play([move(5)])
    arrows.clear()
    expect(shown()).toHaveLength(0)
    expect(timers.pending()).toEqual([])
  })

  it('keeps one value for an empty layer, so nothing redraws for it', () => {
    const { arrows, timers, shown } = setup()
    const empty = shown()
    arrows.play([move(6, null)])
    expect(shown()).toBe(empty)
    arrows.play([move(5)])
    timers.advance(ARROW_MS)
    expect(shown()).toBe(empty)
  })
})
