// Goal: a lost connection reconnects or stops exactly as the close-code table says, with 250 ms doubling to 15 s of backoff that only a Welcome resets.
import { describe, expect, it } from 'vitest'
import { nextWait, open, setup, welcome } from '@/test-utils/socketSetup'
import { retryDelay } from './socket'
import type { ConnectionStatus } from './socket'

const closeCodes: [code: number, status: ConnectionStatus, reason: RegExp | null][] = [
  [1001, 'reconnecting', null],
  [1006, 'reconnecting', null],
  [1013, 'reconnecting', null],
  // Not in the table: treated as a lost connection.
  [1005, 'reconnecting', null],
  [1011, 'reconnecting', null],
  [1012, 'reconnecting', null],
  [4002, 'reconnecting', null],
  [4999, 'reconnecting', null],
  [4000, 'replaced', /another window/],
  [4001, 'stopped', /removed from this game/],
  [1000, 'stopped', /ended the connection/],
  [1002, 'stopped', /protocol error/],
  [1007, 'stopped', /could not read/],
  [1008, 'stopped', /policy violation/],
  [1009, 'stopped', /too big/],
]

describe.each([
  ['after the Welcome', true],
  ['before the Welcome', false],
])('a close %s', (_when, welcomed) => {
  it.each(closeCodes)('with code %i leaves the status %s', (code, status, reason) => {
    const t = setup()
    t.socket.connect()
    t.net.last.open()
    if (welcomed) t.net.last.receive(welcome)
    t.net.last.serverClose(code)
    expect(t.socket.status).toBe(status)
    if (status === 'reconnecting') {
      expect(t.timers.pending()).toEqual([250])
      t.timers.advance(250)
      expect(t.net.sockets).toHaveLength(2)
      expect(t.lastStatus()).toMatchObject({ status: 'reconnecting', attempt: 1, reason: null })
    } else {
      expect(t.timers.pending()).toEqual([])
      t.timers.advance(120_000)
      expect(t.net.sockets).toHaveLength(1)
      expect(t.lastStatus()).toMatchObject({ status, reason: expect.stringMatching(reason ?? /./) })
      // The socket is already closed by the server, so there is nothing for the client to close.
      expect(t.net.last.closedByClient).toBeNull()
    }
  })
})

describe('closes that explain themselves', () => {
  it.each([
    [1008, 'rate limit exceeded'],
    [1008, 'too many malformed messages'],
    [1002, 'protocol error'],
  ])('code %i keeps the server reason "%s" in the message for the player', (code, serverReason) => {
    const t = setup()
    open(t)
    t.net.last.serverClose(code, serverReason)
    expect(t.lastStatus()).toMatchObject({
      status: 'stopped',
      reason: expect.stringContaining(`(${serverReason})`),
    })
  })
})

describe('backoff', () => {
  it('waits 250 ms, doubling to 15 s, after each failed attempt in a row', () => {
    const t = setup()
    t.socket.connect()
    const waits: number[] = []
    for (let failure = 1; failure <= 9; failure += 1) {
      t.net.last.fail()
      const wait = nextWait(t)
      waits.push(wait)
      t.timers.advance(wait - 1)
      expect(t.net.sockets).toHaveLength(failure)
      t.timers.advance(1)
      expect(t.net.sockets).toHaveLength(failure + 1)
    }
    expect(waits).toEqual([250, 500, 1000, 2000, 4000, 8000, 15_000, 15_000, 15_000])
    const attempts = t.events.flatMap((event) =>
      event.type === 'status' && event.status === 'reconnecting' ? [event.attempt] : [],
    )
    expect(attempts).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9])
  })

  it('starts again at 250 ms after a Welcome, and not after a socket that only opened', () => {
    const t = setup()
    t.socket.connect()
    for (let failure = 0; failure < 3; failure += 1) {
      t.net.last.fail()
      t.timers.advance(nextWait(t))
    }
    // Opened but never welcomed: the next wait keeps growing.
    t.net.last.open()
    t.net.last.serverClose(1006)
    expect(nextWait(t)).toBe(2000)
    t.timers.advance(2000)
    t.net.last.open()
    t.net.last.receive(welcome)
    expect(t.lastStatus()).toMatchObject({ status: 'open', attempt: 0 })
    t.net.last.serverClose(1006)
    expect(nextWait(t)).toBe(250)
    expect(t.lastStatus()).toMatchObject({ status: 'reconnecting', attempt: 1 })
  })

  it('draws its jitter from the injected random source, once per retry', () => {
    const draws = [1, 0.5, 0]
    const t = setup({ random: () => draws.shift() ?? 0 })
    t.socket.connect()
    const waits: number[] = []
    for (let failure = 0; failure < 3; failure += 1) {
      t.net.last.fail()
      waits.push(nextWait(t))
      t.timers.advance(nextWait(t))
    }
    expect(waits).toEqual([125, 375, 1000])
  })

  it.each([
    [1, 0, 250],
    [1, 1, 125],
    [2, 0.5, 375],
    [3, 0.25, 875],
    [7, 0, 15_000],
    [7, 1, 7500],
    [64, 0, 15_000],
    [5000, 0, 15_000],
    [1, -5, 250],
    [1, 9, 125],
    [1, Number.NaN, 250],
  ])('retryDelay(%i failures, random %f) is %i ms', (failures, random, expected) => {
    expect(retryDelay(failures, random)).toBe(expected)
  })

  it('never goes below half or above the 15 s ceiling, and never shrinks as failures pile up', () => {
    for (const random of [0, 0.1, 0.5, 0.9, 0.999999]) {
      let previous = 0
      for (let failures = 1; failures <= 40; failures += 1) {
        const wait = retryDelay(failures, random)
        expect(wait).toBeGreaterThanOrEqual(125)
        expect(wait).toBeLessThanOrEqual(15_000)
        expect(wait).toBeGreaterThanOrEqual(previous)
        previous = wait
      }
    }
  })
})
