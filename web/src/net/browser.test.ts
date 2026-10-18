// Goal: a GameSocket built with no injections works on the browser's own WebSocket, window timers and page origin.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { PROTOCOL_VERSION } from '@/protocol/generated'
import { GameSocket, browserSocket } from './socket'
import type { SocketEvent } from './socket'
import { browserTimers } from './timers'

/** Stands in for the browser's WebSocket class and remembers every instance. */
class StubWebSocket {
  static instances: StubWebSocket[] = []
  readonly url: string
  readonly sent: string[] = []
  closedWith: [number, string] | null = null
  onopen: (() => void) | null = null
  onmessage: ((event: { data: unknown }) => void) | null = null
  onclose: ((event: { code: number; reason: string }) => void) | null = null

  constructor(url: string) {
    this.url = url
    StubWebSocket.instances.push(this)
  }

  send(text: string): void {
    this.sent.push(text)
  }

  close(code: number, reason: string): void {
    this.closedWith = [code, reason]
  }
}

const welcome = {
  type: 'Welcome',
  protocol_version: PROTOCOL_VERSION,
  game: 'g',
  player: 1,
  seats: 3,
  auto_pass: true,
  yielding: null,
}

beforeEach(() => {
  StubWebSocket.instances = []
  vi.stubGlobal('WebSocket', StubWebSocket)
  vi.useFakeTimers()
})

afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

function connected() {
  const events: SocketEvent[] = []
  const socket = new GameSocket({
    game: 'g',
    token: 't',
    name: 'Ana',
    onEvent: (event) => events.push(event),
  })
  socket.connect()
  return { socket, events }
}

describe('the browser defaults', () => {
  it('opens a WebSocket on the page origin and joins when it opens', () => {
    connected()
    const [ws] = StubWebSocket.instances
    expect(ws?.url).toBe(`ws://${window.location.host}/ws/g/t`)
    ws?.onopen?.()
    expect(ws?.sent.map((text): unknown => JSON.parse(text))).toEqual([
      { type: 'Join', protocol_version: PROTOCOL_VERSION, game: 'g', name: 'Ana' },
    ])
  })

  it('delivers frames, reports open on the Welcome and closes with 1000 on disconnect', () => {
    const { socket, events } = connected()
    const ws = StubWebSocket.instances[0]
    ws?.onopen?.()
    ws?.onmessage?.({ data: JSON.stringify(welcome) })
    expect(events.at(-1)).toMatchObject({ type: 'status', status: 'open' })
    expect(events.some((event) => event.type === 'message' && event.message.type === 'Welcome')).toBe(true)
    socket.disconnect()
    expect(ws?.closedWith).toEqual([1000, 'client closing'])
  })

  it('passes a binary frame on as an error, not an exception', () => {
    const { events } = connected()
    const ws = StubWebSocket.instances[0]
    ws?.onopen?.()
    ws?.onmessage?.({ data: new ArrayBuffer(4) })
    expect(events.at(-1)).toMatchObject({ type: 'frame_error', error: { kind: 'not_text' } })
  })

  it('retries on window timers after a lost connection, doubling the wait', () => {
    const random = vi.spyOn(Math, 'random').mockReturnValue(0)
    const { socket, events } = connected()
    StubWebSocket.instances[0]?.onclose?.({ code: 1006, reason: '' })
    expect(events.at(-1)).toMatchObject({ status: 'reconnecting', attempt: 1 })
    vi.advanceTimersByTime(249)
    expect(StubWebSocket.instances).toHaveLength(1)
    vi.advanceTimersByTime(1)
    expect(StubWebSocket.instances).toHaveLength(2)
    StubWebSocket.instances[1]?.onclose?.({ code: 1006, reason: '' })
    vi.advanceTimersByTime(499)
    expect(StubWebSocket.instances).toHaveLength(2)
    vi.advanceTimersByTime(1)
    expect(StubWebSocket.instances).toHaveLength(3)
    random.mockRestore()
    socket.disconnect()
    vi.advanceTimersByTime(60_000)
    expect(StubWebSocket.instances).toHaveLength(3)
  })

  it('wires the raw socket to the handlers and back', () => {
    const calls: string[] = []
    const raw = browserSocket('ws://x/ws/g/t', {
      open: () => calls.push('open'),
      message: (data) => calls.push(`message:${String(data)}`),
      close: (code, reason) => calls.push(`close:${code}:${reason}`),
    })
    const ws = StubWebSocket.instances[0]
    ws?.onopen?.()
    ws?.onmessage?.({ data: 'hello' })
    ws?.onclose?.({ code: 1001, reason: 'bye' })
    raw.send('out')
    raw.close(1000, 'done')
    expect(calls).toEqual(['open', 'message:hello', 'close:1001:bye'])
    expect(ws?.sent).toEqual(['out'])
    expect(ws?.closedWith).toEqual([1000, 'done'])
  })

  it('has timers that run, cancel and tell the time', () => {
    const fired: string[] = []
    browserTimers.setTimeout(() => fired.push('a'), 100)
    const b = browserTimers.setTimeout(() => fired.push('b'), 100)
    browserTimers.clearTimeout(b)
    const before = browserTimers.now()
    vi.advanceTimersByTime(100)
    expect(fired).toEqual(['a'])
    expect(browserTimers.now()).toBeGreaterThanOrEqual(before)
    expect(typeof before).toBe('number')
  })
})
