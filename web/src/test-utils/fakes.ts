import { isRecord } from '@/net/check'
import type { RawSocket, RawSocketHandlers, SocketFactory, Timers } from '@/net/socket'

/** A manual clock: a timer fires only when `advance` moves time past it. */
export class FakeTimers implements Timers {
  private now = 0
  private nextId = 1
  private readonly queue = new Map<number, { at: number; callback: () => void }>()

  setTimeout(callback: () => void, ms: number): number {
    const id = this.nextId
    this.nextId += 1
    this.queue.set(id, { at: this.now + ms, callback })
    return id
  }

  clearTimeout(id: number): void {
    this.queue.delete(id)
  }

  /** Milliseconds until each waiting timer fires, soonest first. */
  pending(): number[] {
    return [...this.queue.values()].map((timer) => timer.at - this.now).sort((a, b) => a - b)
  }

  /** Moves time forward and fires every timer that comes due on the way, including ones those timers set. */
  advance(ms: number): void {
    const target = this.now + ms
    for (;;) {
      const due = [...this.queue.entries()]
        .filter(([, timer]) => timer.at <= target)
        .sort(([idA, a], [idB, b]) => a.at - b.at || idA - idB)[0]
      if (due === undefined) break
      const [id, timer] = due
      this.queue.delete(id)
      this.now = Math.max(this.now, timer.at)
      timer.callback()
    }
    this.now = target
  }
}

/** A socket the test drives by hand: it records what the client writes and delivers what the test says. */
export class FakeSocket implements RawSocket {
  readonly url: string
  /** Frames the client wrote, as text, in order. */
  readonly frames: string[] = []
  /** The code and reason of the client's own close call, if it made one. */
  closedByClient: { code: number; reason: string } | null = null
  private readonly handlers: RawSocketHandlers
  private phase: 'connecting' | 'open' | 'closed' = 'connecting'

  constructor(url: string, handlers: RawSocketHandlers) {
    this.url = url
    this.handlers = handlers
  }

  send(text: string): void {
    // A browser throws when a socket is written before it opened and drops writes after a close.
    if (this.phase === 'connecting') throw new Error('InvalidStateError: the socket is still connecting')
    if (this.phase === 'open') this.frames.push(text)
  }

  close(code: number, reason: string): void {
    this.closedByClient = { code, reason }
    this.phase = 'closed'
  }

  /** What the client wrote, decoded. */
  get sent(): unknown[] {
    return this.frames.map((frame): unknown => JSON.parse(frame))
  }

  /** The `type` of everything the client wrote, in order. */
  get sentTypes(): string[] {
    return this.sent.map((message) => (isRecord(message) ? String(message.type) : '?'))
  }

  /** The handshake completes. */
  open(): void {
    this.phase = 'open'
    this.handlers.open()
  }

  /** The server sends a frame: text as it is, anything else as JSON. */
  receive(message: unknown): void {
    this.handlers.message(typeof message === 'string' ? message : JSON.stringify(message))
  }

  /** The server sends `data` untouched, such as a binary frame. */
  receiveRaw(data: unknown): void {
    this.handlers.message(data)
  }

  /** The server closes the connection with `code`. */
  serverClose(code: number, reason = ''): void {
    this.phase = 'closed'
    this.handlers.close(code, reason)
  }

  /** The network fails: a refused handshake or a dropped line, which browsers report as 1006. */
  fail(): void {
    this.serverClose(1006)
  }
}

/** The factory the client opens sockets with, remembering every one it made. */
export class FakeNetwork {
  readonly sockets: FakeSocket[] = []

  readonly factory: SocketFactory = (url, handlers) => {
    const socket = new FakeSocket(url, handlers)
    this.sockets.push(socket)
    return socket
  }

  /** The newest socket. */
  get last(): FakeSocket {
    const socket = this.sockets.at(-1)
    if (socket === undefined) throw new Error('no socket was opened')
    return socket
  }
}
