// Goal: a `Presence` message sets the seat's `connected` flag in the view and starts, restarts or clears its away clock, which ticks at every whole second on the session's own timers and stops at zero; a reconnect clears it, a snapshot starts the clocks afresh, and a message for a seat that is connected keeps the view as it was.
import { describe, expect, it } from 'vitest'
import type { ServerMessage } from '@/protocol/generated'
import { startSession } from '@/test-utils/harness'
import { lateJoin } from '@/test-utils/transcripts'
import { createAwayClocks } from './awayClocks'
import type { AwayTimes } from './awayClocks'
import { FakeTimers } from '@/test-utils/fakes'

const presence = (seat: number, connected: boolean, deadlineMs?: number): ServerMessage => ({
  type: 'Presence',
  seat,
  connected,
  ...(deadlineMs === undefined ? {} : { deadline_ms: deadlineMs }),
})

/** A session that has caught up with the recorded late join of seat 0 at a two-seat table. */
function caughtUp() {
  const h = startSession()
  h.join(lateJoin.seat, 2)
  h.deliver(...lateJoin.messages.filter((message) => message.type !== 'Welcome'))
  return h
}

const flags = (h: ReturnType<typeof caughtUp>): boolean[] =>
  h.state().view?.players.map((player) => player.connected) ?? []

describe('the session', () => {
  it.each([
    {
      name: 'away with a clock',
      message: presence(1, false, 12_000),
      view: [true, false],
      away: { 1: 12_000 },
    },
    { name: 'away without a clock', message: presence(1, false), view: [true, false], away: { 1: null } },
    { name: 'back again', message: presence(1, true), view: [true, true], away: {} },
  ])('$name', ({ message, view, away }) => {
    const h = caughtUp()
    h.deliver(message)
    expect(flags(h)).toEqual(view)
    expect(h.state().away).toEqual(away)
  })

  it('keeps the view when nothing changed', () => {
    const h = caughtUp()
    const before = h.state().view
    h.deliver(presence(1, true))
    expect(h.state().view).toBe(before)
  })

  it('clears the clock on a reconnect and on a snapshot', () => {
    const h = caughtUp()
    h.deliver(presence(1, false, 5_000))
    h.deliver(presence(1, true))
    expect(h.state().away).toEqual({})
    h.deliver(presence(1, false, 5_000))
    const snapshot = lateJoin.messages.find((message) => message.type === 'Snapshot')
    if (snapshot === undefined) throw new Error('the transcript has a snapshot')
    h.deliver(snapshot)
    expect(h.state().away).toEqual({})
  })
})

describe('the away clocks', () => {
  function clocks() {
    const timers = new FakeTimers()
    const heard: AwayTimes[] = []
    return { timers, heard, away: createAwayClocks(timers, (away) => heard.push(away)) }
  }

  it.each([
    { deadline: 3_000, after: 1_000, left: 2_000 },
    { deadline: 2_500, after: 500, left: 2_000 },
    { deadline: 1_000, after: 1_000, left: 0 },
    { deadline: 1_000, after: 5_000, left: 0 },
  ])('counts $deadline ms down to $left after $after ms', ({ deadline, after, left }) => {
    const { timers, heard, away } = clocks()
    away.presence(2, false, deadline)
    timers.advance(after)
    expect(heard.at(-1)).toEqual({ 2: left })
  })

  it('ticks once per whole second and leaves no timer at zero', () => {
    const { timers, heard, away } = clocks()
    away.presence(0, false, 2_500)
    timers.advance(3_000)
    expect(heard.map((times) => times[0])).toEqual([2_500, 2_000, 1_000, 0])
    expect(timers.pending()).toEqual([])
  })

  it('keeps one timer for several seats and none once all are back', () => {
    const { timers, away } = clocks()
    away.presence(0, false, 4_000)
    away.presence(1, false, 9_000)
    expect(timers.pending()).toHaveLength(1)
    away.presence(0, true, undefined)
    away.presence(1, true, undefined)
    expect(timers.pending()).toEqual([])
  })
})
