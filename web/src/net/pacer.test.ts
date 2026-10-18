// Goal: outgoing messages leave in a burst and then at a fixed rate, in order and none lost, so a client that answers as fast as it is asked never goes over the server's rate limit; the limit replica itself is checked first.
import { describe, expect, it } from 'vitest'
import type { ClientMessage } from '@/protocol/generated'
import { FakeTimers } from '@/test-utils/fakes'
import { withinServerLimit } from '@/test-utils/serverLimit'
import { setup, open } from '@/test-utils/socketSetup'
import { MAX_WAITING, Pacer, SEND_BURST, SEND_PER_SECOND } from './pacer'

const chat = (n: number): ClientMessage => ({ type: 'Chat', text: String(n) })

function pacerSetup(accept: (n: number) => boolean = () => true) {
  const timers = new FakeTimers()
  const written: { n: number; at: number }[] = []
  const pacer = new Pacer(timers, (message) => {
    if (message.type !== 'Chat') return false
    const n = Number(message.text)
    if (!accept(n)) return false
    written.push({ n, at: timers.now() })
    return true
  })
  const numbers = (): number[] => written.map((entry) => entry.n)
  return { timers, written, pacer, numbers }
}

describe('the replica of the server limiter', () => {
  const at = (count: number, ms: number): number[] => Array.from({ length: count }, () => ms)

  it.each<[string, number[], boolean]>([
    ['40 at once', at(40, 0), true],
    ['41 at once', at(41, 0), false],
    ['40, then one 50 ms later (20 a second)', [...at(40, 0), 50], true],
    ['40, then one 49 ms later', [...at(40, 0), 49], false],
    [
      '40, then 20 over the next second',
      [...at(40, 0), ...Array.from({ length: 20 }, (_, i) => (i + 1) * 50)],
      true,
    ],
    [
      '40, then 21 over the next second',
      [...at(40, 0), ...Array.from({ length: 21 }, (_, i) => (i + 1) * 47)],
      false,
    ],
    ['a long quiet spell refills only to the burst', [...at(40, 0), ...at(41, 60_000)], false],
    ['nothing at all', [], true],
  ])('%s', (_name, times, allowed) => {
    expect(withinServerLimit(times)).toBe(allowed)
  })
})

describe('the pacer', () => {
  it('has a smaller budget than the server allows', () => {
    expect(SEND_BURST).toBeLessThan(40)
    expect(SEND_PER_SECOND).toBeLessThan(20)
  })

  it('writes the burst at once and holds the rest back, in order and all accepted', () => {
    const t = pacerSetup()
    for (let n = 0; n < 100; n += 1) expect(t.pacer.push(chat(n))).toBe(true)
    expect(t.numbers()).toEqual(Array.from({ length: SEND_BURST }, (_, n) => n))
    expect(t.timers.pending()).toHaveLength(1)
    t.timers.advance(60_000)
    expect(t.numbers()).toEqual(Array.from({ length: 100 }, (_, n) => n))
    expect(t.timers.pending()).toEqual([])
  })

  it('releases one message every 67 ms once the burst is spent', () => {
    const t = pacerSetup()
    for (let n = 0; n < SEND_BURST + 3; n += 1) t.pacer.push(chat(n))
    expect(t.written).toHaveLength(SEND_BURST)
    t.timers.advance(66)
    expect(t.written).toHaveLength(SEND_BURST)
    t.timers.advance(1)
    expect(t.written).toHaveLength(SEND_BURST + 1)
    t.timers.advance(67)
    expect(t.written).toHaveLength(SEND_BURST + 2)
    t.timers.advance(67)
    expect(t.written).toHaveLength(SEND_BURST + 3)
    expect(t.timers.pending()).toEqual([])
  })

  it('puts a late message behind the ones that already wait', () => {
    const t = pacerSetup()
    for (let n = 0; n < SEND_BURST + 1; n += 1) t.pacer.push(chat(n))
    t.timers.advance(10)
    t.pacer.push(chat(1000))
    t.timers.advance(10_000)
    expect(t.numbers().slice(-2)).toEqual([SEND_BURST, 1000])
  })

  it('writes at once when it is quiet again and nobody waits', () => {
    const t = pacerSetup()
    for (let n = 0; n < SEND_BURST; n += 1) t.pacer.push(chat(n))
    t.timers.advance(5000)
    t.pacer.push(chat(500))
    expect(t.written.at(-1)).toEqual({ n: 500, at: 5000 })
  })

  it.each([
    [1000, SEND_PER_SECOND],
    [2000, SEND_BURST],
    [10_000, SEND_BURST],
    [0, 0],
  ])('after %i ms of quiet it lets %i messages through at once', (quiet, allowed) => {
    const t = pacerSetup()
    for (let n = 0; n < SEND_BURST; n += 1) t.pacer.push(chat(n))
    t.timers.advance(quiet)
    const before = t.written.length
    for (let n = 0; n < 100; n += 1) t.pacer.push(chat(1000 + n))
    expect(t.written.length - before).toBe(allowed)
  })

  it(`refuses a message once ${MAX_WAITING} already wait, and takes messages again when some have left`, () => {
    const t = pacerSetup()
    for (let n = 0; n < SEND_BURST + MAX_WAITING; n += 1) expect(t.pacer.push(chat(n))).toBe(true)
    expect(t.pacer.push(chat(9999))).toBe(false)
    expect(t.pacer.push(chat(9998))).toBe(false)
    t.timers.advance(70)
    expect(t.pacer.push(chat(9997))).toBe(true)
    t.timers.advance(60_000)
    expect(t.numbers().filter((n) => n >= 9000)).toEqual([9997])
    expect(t.written).toHaveLength(SEND_BURST + MAX_WAITING + 1)
  })

  it('forgets what waits on reset, cancels its timer and starts with a full budget', () => {
    const t = pacerSetup()
    for (let n = 0; n < 100; n += 1) t.pacer.push(chat(n))
    t.pacer.reset()
    expect(t.timers.pending()).toEqual([])
    t.timers.advance(60_000)
    expect(t.written).toHaveLength(SEND_BURST)
    for (let n = 0; n < SEND_BURST; n += 1) t.pacer.push(chat(1000 + n))
    expect(t.written).toHaveLength(2 * SEND_BURST)
    expect(t.written.at(-1)?.at).toBe(60_000)
  })

  it('tells the caller when an immediate write fails, and keeps going after a queued write fails', () => {
    const closed = pacerSetup((n) => n !== 3)
    for (let n = 0; n < 3; n += 1) expect(closed.pacer.push(chat(n))).toBe(true)
    expect(closed.pacer.push(chat(3))).toBe(false)
    expect(closed.pacer.push(chat(4))).toBe(true)

    const later = pacerSetup((n) => n !== SEND_BURST + 1)
    for (let n = 0; n < SEND_BURST + 4; n += 1) later.pacer.push(chat(n))
    later.timers.advance(60_000)
    expect(later.numbers()).toEqual(
      Array.from({ length: SEND_BURST + 4 }, (_, n) => n).filter((n) => n !== SEND_BURST + 1),
    )
  })

  it('never goes over the server limit, whatever the producer does, and loses nothing it accepted', () => {
    let seed = 7
    const random = (): number => {
      seed = (seed * 1_103_515_245 + 12_345) % 2_147_483_648
      return seed / 2_147_483_648
    }
    for (let round = 0; round < 40; round += 1) {
      const t = pacerSetup()
      const accepted: number[] = []
      let next = 0
      for (let step = 0; step < 60; step += 1) {
        const group = 1 + Math.floor(random() * 80)
        for (let k = 0; k < group; k += 1) {
          if (t.pacer.push(chat(next))) accepted.push(next)
          next += 1
        }
        t.timers.advance(Math.floor(random() * 3000))
      }
      t.timers.advance(120_000)
      expect(withinServerLimit(t.written.map((entry) => entry.at))).toBe(true)
      expect(
        withinServerLimit(
          t.written.map((entry) => entry.at),
          SEND_BURST,
          SEND_PER_SECOND,
        ),
      ).toBe(true)
      expect(t.numbers()).toEqual(accepted)
    }
  })
})

describe('the socket with the pacer in front', () => {
  const sendMany = (t: ReturnType<typeof setup>, count: number, from = 0): boolean[] =>
    Array.from({ length: count }, (_, i) => t.socket.send(chat(from + i)))

  it('puts Join first and keeps a burst of 100 in order and within the server limit', () => {
    const t = setup()
    open(t)
    expect(sendMany(t, 100).every(Boolean)).toBe(true)
    t.timers.advance(60_000)
    const types = t.net.last.sentTypes
    expect(types[0]).toBe('Join')
    expect(types.slice(1)).toEqual(Array.from({ length: 100 }, () => 'Chat'))
    expect(t.net.last.sent.slice(1)).toEqual(Array.from({ length: 100 }, (_, n) => chat(n)))
    expect(withinServerLimit(t.net.last.times)).toBe(true)
  })

  it('spends one token on Join, so 29 messages follow it at once', () => {
    const t = setup()
    open(t)
    sendMany(t, 40)
    expect(t.net.last.frames).toHaveLength(SEND_BURST)
  })

  it('drops what waits when the connection is lost, and leaves only the retry timer', () => {
    const t = setup()
    open(t)
    sendMany(t, 100)
    t.net.last.fail()
    expect(t.timers.pending()).toEqual([250])
    t.timers.advance(250)
    t.net.last.open()
    t.timers.advance(60_000)
    expect(t.net.last.sentTypes).toEqual(['Join'])
  })

  it('drops what waits on disconnect and leaves no timer behind', () => {
    const t = setup()
    open(t)
    sendMany(t, 100)
    const delivered = t.net.last.frames.length
    t.socket.disconnect()
    expect(t.timers.pending()).toEqual([])
    t.timers.advance(60_000)
    expect(t.net.last.frames).toHaveLength(delivered)
  })

  it('gives each new connection a full budget', () => {
    const t = setup()
    open(t)
    sendMany(t, 100)
    t.net.last.serverClose(1001)
    t.timers.advance(250)
    t.net.last.open()
    t.net.last.receive({
      type: 'Welcome',
      protocol_version: 1,
      game: 'g',
      player: 0,
      seats: 2,
      auto_pass: true,
      yielding: null,
    })
    sendMany(t, 100)
    expect(t.net.last.frames).toHaveLength(SEND_BURST)
  })

  it('answers false once the queue is full, as it does for a closed socket', () => {
    const t = setup()
    open(t)
    const results = sendMany(t, SEND_BURST + MAX_WAITING + 5)
    expect(results.filter((ok) => !ok)).toHaveLength(6)
    t.net.last.fail()
    expect(t.socket.send(chat(0))).toBe(false)
  })
})
