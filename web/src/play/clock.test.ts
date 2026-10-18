// Goal: a decision with a deadline starts a clock that counts down from the moment its message arrived, on the session's own timers; it ticks whenever the whole seconds left change, ends the prompt at zero, stops with an answer, a replacement, a snapshot or a lost connection, and leaves no timer behind; a decision without a deadline never has one.
import { describe, expect, it } from 'vitest'
import type { ServerMessage } from '@/protocol/generated'
import { decisionFor } from '@/test-utils/decisions'
import { pass, priorityDecision, startSession } from '@/test-utils/harness'
import type { Harness } from '@/test-utils/harness'
import { lateJoin } from '@/test-utils/transcripts'

const clocked = (id: number, deadlineMs: number): ServerMessage => ({
  type: 'Decision',
  id,
  decision: decisionFor('Priority'),
  deadline_ms: deadlineMs,
})

/** A joined session, and how many timers the connection itself keeps. */
function joined(): Harness & { baseline: number } {
  const h = startSession()
  h.join()
  return Object.assign(h, { baseline: h.timers.pending().length })
}

const timersLeft = (h: Harness & { baseline: number }): number => h.timers.pending().length - h.baseline

describe('starting', () => {
  it('begins at the full time when the message arrives', () => {
    const h = joined()
    h.deliver(clocked(5, 10_000))
    expect(h.state().pending).toMatchObject({ id: 5, deadlineMs: 10_000 })
    expect(h.state().clock).toEqual({ id: 5, totalMs: 10_000, leftMs: 10_000 })
    expect(timersLeft(h)).toBe(1)
  })

  it('counts from the arrival, on the manual clock, not from when the server sent it', () => {
    const h = joined()
    h.timers.advance(4_000)
    h.deliver(clocked(5, 10_000))
    h.timers.advance(3_000)
    expect(h.state().clock).toMatchObject({ totalMs: 10_000, leftMs: 7_000 })
  })

  it('has no clock for a decision without a deadline', () => {
    const h = joined()
    h.deliver(priorityDecision(5))
    expect(h.state().pending).toMatchObject({ id: 5 })
    expect(h.state().pending).not.toHaveProperty('deadlineMs')
    expect(h.state().clock).toBeNull()
    expect(timersLeft(h)).toBe(0)
  })

  it('starts from what is left after a reconnect: the same decision with less time', () => {
    const h = joined()
    h.deliver(clocked(5, 10_000))
    h.timers.advance(4_000)
    h.net.last.fail()
    expect(h.state()).toMatchObject({ pending: null, clock: null })
    h.timers.advance(250)
    h.open()
    h.deliver(welcome(), snapshot(), clocked(5, 5_000))
    expect(h.state().clock).toEqual({ id: 5, totalMs: 5_000, leftMs: 5_000 })
  })
})

describe('ticking', () => {
  it('ticks once a second from a whole number of seconds', () => {
    const h = joined()
    h.deliver(clocked(5, 3_000))
    expect(h.timers.pending().at(-1)).toBe(1_000)
    const seen: number[] = []
    for (let n = 0; n < 2; n += 1) {
      h.timers.advance(1_000)
      seen.push(h.state().clock?.leftMs ?? -1)
    }
    expect(seen).toEqual([2_000, 1_000])
  })

  it('wakes when the whole seconds left change, not on a fixed beat, from a part of a second', () => {
    const h = joined()
    h.deliver(clocked(5, 9_500))
    expect(h.timers.pending().at(-1)).toBe(500)
    h.timers.advance(500)
    expect(h.state().clock?.leftMs).toBe(9_000)
    expect(h.timers.pending().at(-1)).toBe(1_000)
  })

  it('keeps the same decision on the screen while it ticks', () => {
    const h = joined()
    h.deliver(clocked(5, 10_000))
    const before = h.state().pending
    h.timers.advance(5_000)
    expect(h.state().pending).toBe(before)
  })

  it('holds one timer at a time', () => {
    const h = joined()
    h.deliver(clocked(5, 10_000))
    for (let n = 0; n < 9; n += 1) {
      h.timers.advance(1_000)
      expect(timersLeft(h)).toBe(1)
    }
  })
})

describe('ending', () => {
  it('ends the prompt when the time is up, as the server has answered for the seat', () => {
    const h = joined()
    h.deliver(clocked(5, 10_000))
    h.timers.advance(9_999)
    expect(h.state().pending).not.toBeNull()
    h.timers.advance(1)
    expect(h.state()).toMatchObject({ pending: null, clock: null })
    expect(timersLeft(h)).toBe(0)
    expect(h.sentOf('Answer')).toEqual([])
  })

  it('ends at once for a decision that has no time left', () => {
    const h = joined()
    h.deliver(clocked(5, 0))
    expect(h.state()).toMatchObject({ pending: null, clock: null })
    expect(timersLeft(h)).toBe(0)
  })

  it('stops with an answer and leaves nothing running', () => {
    const h = joined()
    h.deliver(clocked(5, 10_000))
    expect(h.session.answer(pass)).toBe(true)
    expect(h.state()).toMatchObject({ pending: null, clock: null })
    expect(timersLeft(h)).toBe(0)
    h.timers.advance(20_000)
    expect(h.state().pending).toBeNull()
  })

  it('is replaced by the clock of the next decision, and the old one never fires', () => {
    const h = joined()
    h.deliver(clocked(5, 10_000))
    h.timers.advance(3_000)
    h.deliver(clocked(6, 20_000))
    expect(h.state().clock).toEqual({ id: 6, totalMs: 20_000, leftMs: 20_000 })
    expect(timersLeft(h)).toBe(1)
    h.timers.advance(8_000) // past the first decision's end
    expect(h.state().pending).toMatchObject({ id: 6 })
    h.timers.advance(12_000)
    expect(h.state().pending).toBeNull()
  })

  it('is dropped by a decision without a deadline', () => {
    const h = joined()
    h.deliver(clocked(5, 10_000))
    h.deliver(priorityDecision(6))
    expect(h.state().clock).toBeNull()
    expect(timersLeft(h)).toBe(0)
    h.timers.advance(60_000)
    expect(h.state().pending).toMatchObject({ id: 6 })
  })

  it('stops with a snapshot, which hands over the real state', () => {
    const h = joined()
    h.deliver(clocked(5, 10_000))
    h.deliver(snapshot())
    expect(h.state()).toMatchObject({ pending: null, clock: null })
    expect(timersLeft(h)).toBe(0)
  })

  it('stops when the connection is lost, and for good when the game is over', () => {
    const lost = joined()
    lost.deliver(clocked(5, 10_000))
    lost.net.last.fail()
    expect(lost.state().clock).toBeNull()

    const over = joined()
    over.deliver(clocked(5, 10_000))
    over.deliver({ type: 'GameOver', result: 'Draw' })
    expect(over.state()).toMatchObject({ pending: null, clock: null })
    expect(timersLeft(over)).toBe(0)
  })

  it('stops when the page lets go of the session', () => {
    const h = joined()
    h.deliver(clocked(5, 10_000))
    h.session.disconnect()
    expect(h.state().clock).toBeNull()
    expect(timersLeft(h)).toBe(0)
  })
})

function welcome(): ServerMessage {
  const message = lateJoin.raw.find((item) => (item as { type?: string }).type === 'Welcome')
  return message as ServerMessage
}

function snapshot(): ServerMessage {
  const message = lateJoin.raw.find((item) => (item as { type?: string }).type === 'Snapshot')
  return message as ServerMessage
}
