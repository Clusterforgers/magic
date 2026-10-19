/**
 * One seat's WebSocket to `mtg-server`: sends `Join` first, parses every frame at the boundary and reconnects with
 * exponential backoff. It knows nothing of the game; `play/session.ts` listens to its events.
 */
import { PROTOCOL_VERSION } from '@/protocol/generated'
import type { ClientMessage, ErrorCode, ServerMessage } from '@/protocol/generated'
import { Pacer } from './pacer'
import { parseFrame } from './parse'
import type { FrameError } from './parse'
import { browserTimers } from './timers'
import type { Timers } from './timers'

/** Where the connection stands: `open` means the server answered `Join` with `Welcome`. */
export type ConnectionStatus =
  'idle' | 'connecting' | 'open' | 'reconnecting' | 'replaced' | 'stopped' | 'closed'

/** What a `GameSocket` reports to its owner. */
export type SocketEvent =
  | {
      type: 'status'
      status: ConnectionStatus
      /** Failed attempts in a row since the last `Welcome`; 0 while open. */
      attempt: number
      /** Why the socket stopped, in words for a player; null when it did not stop for a reason. */
      reason: string | null
    }
  | { type: 'message'; message: ServerMessage }
  | { type: 'frame_error'; error: FrameError }

/** The callbacks a raw socket calls; they are never called before the factory returns. */
export interface RawSocketHandlers {
  open(): void
  message(data: unknown): void
  close(code: number, reason: string): void
}

/** The part of a WebSocket the client uses. */
export interface RawSocket {
  send(text: string): void
  close(code: number, reason: string): void
}

/** Opens a socket to `url`; tests pass a fake. */
export type SocketFactory = (url: string, handlers: RawSocketHandlers) => RawSocket

/** A browser WebSocket. */
export const browserSocket: SocketFactory = (url, handlers) => {
  const ws = new WebSocket(url)
  ws.onopen = () => handlers.open()
  ws.onmessage = (event) => handlers.message(event.data)
  ws.onclose = (event) => handlers.close(event.code, event.reason)
  return { send: (text) => ws.send(text), close: (code, reason) => ws.close(code, reason) }
}

/** Options for a `GameSocket`; `game`, `name` and `onEvent` have no default. */
export interface GameSocketOptions {
  game: string
  /** The seat token of a seat link. Left out, the socket joins through the session cookie (a signed-in user's seat). */
  token?: string
  /** The display name sent in `Join`. */
  name: string
  onEvent(event: SocketEvent): void
  createSocket?: SocketFactory
  timers?: Timers
  /** Jitter source in [0, 1); defaults to `Math.random`. */
  random?: () => number
  /** Origin of the page, such as `https://play.example`; defaults to `window.location.origin`. */
  origin?: string
}

/** The first retry waits this long, in milliseconds, and each failure doubles it. */
export const FIRST_RETRY_MS = 250
/** No retry waits longer than this, in milliseconds. */
export const LONGEST_RETRY_MS = 15_000
const JITTER = 0.5
const NORMAL_CLOSE = 1000
const REPLACED_CODE = 4000
const REMOVED_CODE = 4001
const CLOSED_CODE = 4002

/** The wait before retry number `failures` (1 is the first): 250 ms doubling to 15 s, minus up to half as jitter. */
export function retryDelay(failures: number, random: number): number {
  const ceiling = Math.min(LONGEST_RETRY_MS, FIRST_RETRY_MS * 2 ** (failures - 1))
  const spread = Number.isFinite(random) ? Math.min(1, Math.max(0, random)) : 0
  return Math.round(ceiling * (1 - JITTER * spread))
}

/**
 * The WebSocket address of a seat on `origin`, with ws or wss to match its scheme: `/ws/{game}/{token}` for a seat
 * link, `/ws/{game}` without a token, where the session cookie of the upgrade request picks the signed-in user's seat.
 */
export function socketUrl(game: string, token: string | undefined, origin: string): string {
  const seat = token === undefined ? '' : `/${encodeURIComponent(token)}`
  const url = new URL(`/ws/${encodeURIComponent(game)}${seat}`, origin)
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:'
  return url.href
}

const REPLACED_TEXT = 'This seat was opened in another window or tab, which took it over.'
// The seat was given back or the host removed its player: its token opens nothing now, so retrying cannot help.
const REMOVED_TEXT = 'You were removed from this game.'
// The host ended the lobby: its links and code open nothing any more, so the game cannot come back either.
const CLOSED_TEXT = 'The host closed this game.'

// Close codes after which retrying cannot help; any other code is treated as a lost connection.
const STOP_REASONS: Readonly<Record<number, string>> = {
  1000: 'The server ended the connection',
  1002: 'The server refused the connection as a protocol error',
  1007: 'The server could not read a message this page sent',
  1008: 'The server closed the connection because of a policy violation',
  1009: 'A message was too big for the server',
}

const FATAL_ERRORS: Readonly<Partial<Record<ErrorCode, string>>> = {
  UnsupportedVersion:
    'This page and the server speak different protocol versions. Reload the page; if that does not help, the app needs an update.',
  UnknownGame: 'This game does not exist any more.',
}

/** A seat's connection to its game. */
export class GameSocket {
  private readonly options: GameSocketOptions
  private readonly createSocket: SocketFactory
  private readonly timers: Timers
  private readonly random: () => number
  private readonly pacer: Pacer
  private state: ConnectionStatus = 'idle'
  private raw: RawSocket | null = null
  private opened = false
  // Bumped whenever a socket is replaced or abandoned, so its late events can be told apart and ignored.
  private generation = 0
  private failures = 0
  private retryTimer: number | null = null
  private retired = false

  constructor(options: GameSocketOptions) {
    this.options = options
    this.createSocket = options.createSocket ?? browserSocket
    this.timers = options.timers ?? browserTimers
    this.random = options.random ?? Math.random
    this.pacer = new Pacer(this.timers, (message) => this.write(message))
  }

  /** The current status. */
  get status(): ConnectionStatus {
    return this.state
  }

  /** Starts connecting; does nothing while a connection or a retry is already under way. */
  connect(): void {
    if (this.state === 'connecting' || this.state === 'open' || this.state === 'reconnecting') return
    this.failures = 0
    this.retired = false
    this.setStatus('connecting', null)
    this.openSocket()
  }

  /** Closes with code 1000, cancels any retry and goes back to `idle`. Safe to call at any time. */
  disconnect(): void {
    this.cancelRetry()
    this.generation += 1
    this.closeRaw()
    if (this.state !== 'idle') this.setStatus('idle', null)
  }

  /**
   * Sends one message; false when the socket is not open (or 200 messages already wait), in which case nothing
   * will be sent. A message over the server's rate limit is held back briefly rather than refused.
   */
  send(message: ClientMessage): boolean {
    return this.opened && this.pacer.push(message)
  }

  /** Tells the socket nothing more is needed (the game is over): a lost connection is no longer retried. */
  retire(): void {
    this.retired = true
  }

  private openSocket(): void {
    this.generation += 1
    this.pacer.reset()
    const generation = this.generation
    const live = (): boolean => generation === this.generation
    try {
      this.raw = this.createSocket(socketUrl(this.options.game, this.options.token, this.pageOrigin()), {
        open: () => {
          if (live()) this.onOpen()
        },
        message: (data) => {
          if (live()) this.onFrame(data)
        },
        close: (code, reason) => {
          if (live()) this.onClose(code, reason)
        },
      })
    } catch {
      this.halt('stopped', 'The browser refused to open the connection.')
    }
  }

  private pageOrigin(): string {
    return this.options.origin ?? window.location.origin
  }

  private onOpen(): void {
    if (this.opened) return
    this.opened = true
    this.pacer.push({
      type: 'Join',
      protocol_version: PROTOCOL_VERSION,
      game: this.options.game,
      name: this.options.name,
    })
  }

  private onFrame(data: unknown): void {
    const frame = parseFrame(data)
    if (!frame.ok) {
      this.emit({ type: 'frame_error', error: frame.error })
      return
    }
    const { message } = frame
    const generation = this.generation
    if (message.type === 'Welcome') this.failures = 0
    this.emit({ type: 'message', message })
    // The owner may have closed the socket while handling the message.
    if (generation !== this.generation) return
    if (message.type === 'Welcome') {
      this.setStatus('open', null)
    } else if (message.type === 'Error') {
      const reason = FATAL_ERRORS[message.code]
      if (reason !== undefined) this.halt('stopped', reason)
    }
  }

  private onClose(code: number, reason: string): void {
    this.raw = null
    this.opened = false
    this.pacer.reset()
    this.generation += 1
    if (code === REPLACED_CODE) {
      this.halt('replaced', REPLACED_TEXT)
      return
    }
    if (code === REMOVED_CODE) {
      this.halt('stopped', REMOVED_TEXT)
      return
    }
    if (code === CLOSED_CODE) {
      this.halt('closed', CLOSED_TEXT)
      return
    }
    const stopped = STOP_REASONS[code]
    if (stopped !== undefined) {
      this.halt('stopped', `${stopped}${reason === '' ? '' : ` (${reason})`}.`)
    } else if (this.retired) {
      this.halt('stopped', null)
    } else {
      this.scheduleRetry()
    }
  }

  private scheduleRetry(): void {
    this.failures += 1
    const wait = retryDelay(this.failures, this.random())
    this.retryTimer = this.timers.setTimeout(() => {
      this.retryTimer = null
      if (this.state === 'reconnecting') this.openSocket()
    }, wait)
    this.setStatus('reconnecting', null)
  }

  // Ends the connection for good (until `connect` is called again) and says why.
  private halt(status: 'stopped' | 'replaced' | 'closed', reason: string | null): void {
    if (this.state === 'idle') return
    this.cancelRetry()
    this.generation += 1
    this.closeRaw()
    this.setStatus(status, reason)
  }

  private closeRaw(): void {
    const raw = this.raw
    this.raw = null
    this.opened = false
    this.pacer.reset()
    if (raw === null) return
    try {
      raw.close(NORMAL_CLOSE, 'client closing')
    } catch {
      // The socket was already closing; nothing is left to do for it.
    }
  }

  private cancelRetry(): void {
    if (this.retryTimer === null) return
    this.timers.clearTimeout(this.retryTimer)
    this.retryTimer = null
  }

  private write(message: ClientMessage): boolean {
    if (this.raw === null) return false
    try {
      this.raw.send(JSON.stringify(message))
    } catch {
      return false
    }
    return true
  }

  private setStatus(status: ConnectionStatus, reason: string | null): void {
    this.state = status
    this.emit({ type: 'status', status, attempt: this.failures, reason })
  }

  private emit(event: SocketEvent): void {
    this.options.onEvent(event)
  }
}
