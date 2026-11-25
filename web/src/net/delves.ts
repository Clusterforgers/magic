/**
 * The Delves endpoints of `mtg-server` (docs/server.md, Delves): the caller's runs, a new run, joining by code, the
 * run brought up to date, one action of a member, the campaign content the pages draw, the leaderboard and the
 * socket that follows a run.
 */
import type {
  CreateDelve,
  DelveAction,
  DelveContent,
  DelveLeaderboard,
  DelveList,
  DelveView,
} from '@/protocol/generated'
import type { Fetch } from './api'
import { isRecord } from './check'
import { browserSocket, retryDelay } from './socket'
import type { RawSocket, SocketFactory } from './socket'
import { browserTimers } from './timers'
import type { Timers } from './timers'

/** Why a call failed: the server's code, or `network` when it was not reached. */
export class DelveError extends Error {
  readonly code: string
  readonly status: number

  constructor(code: string, status: number, message: string) {
    super(message)
    this.name = 'DelveError'
    this.code = code
    this.status = status
  }
}

export interface DelvesApi {
  list(): Promise<DelveList>
  create(body: CreateDelve): Promise<DelveView>
  join(code: string): Promise<DelveView>
  get(id: string): Promise<DelveView>
  act(id: string, action: DelveAction): Promise<DelveView>
  content(): Promise<DelveContent>
  leaderboard(): Promise<DelveLeaderboard>
  /** Follows a run over its socket and answers the stop; left out, the page reads the run once. */
  follow?(id: string, handlers: DelveFollowHandlers): () => void
}

function isView(body: unknown): body is DelveView {
  return isRecord(body) && typeof body.id === 'string' && isRecord(body.run)
}

function isList(body: unknown): body is DelveList {
  return isRecord(body) && Array.isArray(body.delves)
}

function isContent(body: unknown): body is DelveContent {
  return isRecord(body) && Array.isArray(body.cards) && Array.isArray(body.bosses)
}

function isLeaderboard(body: unknown): body is DelveLeaderboard {
  return isRecord(body) && Array.isArray(body.boards)
}

/** Builds the client; call it once, or use the shared `delvesApi`. */
export function createDelvesApi(options: { fetch?: Fetch; baseUrl?: string } = {}): DelvesApi {
  const base = `${options.baseUrl ?? ''}/api/delves`

  async function call<T>(
    url: string,
    check: (body: unknown) => body is T,
    init: RequestInit = {},
  ): Promise<T> {
    const send: Fetch = options.fetch ?? ((target, request) => globalThis.fetch(target, request))
    let response: Response
    let text: string
    try {
      response = await send(url, { credentials: 'include', ...init })
      text = await response.text()
    } catch {
      throw new DelveError('network', 0, 'Could not reach the server. Check your connection and try again.')
    }
    let body: unknown = null
    try {
      body = text === '' ? null : JSON.parse(text)
    } catch {
      body = null
    }
    if (!response.ok) {
      const code = isRecord(body) && typeof body.error === 'string' ? body.error : 'unknown'
      const message =
        isRecord(body) && typeof body.message === 'string'
          ? body.message
          : `The server answered with HTTP ${response.status}.`
      throw new DelveError(code, response.status, message)
    }
    if (!check(body))
      throw new DelveError(
        'bad_response',
        response.status,
        'The server sent an answer this page cannot read.',
      )
    return body
  }

  const post = (body: unknown): RequestInit => ({
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })

  return {
    list: () => call(base, isList),
    create: (body) => call(base, isView, post(body)),
    join: (code) => call(`${base}/join/${encodeURIComponent(code.trim())}`, isView, { method: 'POST' }),
    get: (id) => call(`${base}/${encodeURIComponent(id)}`, isView),
    act: (id, action) => call(`${base}/${encodeURIComponent(id)}/act`, isView, post(action)),
    content: () => call(`${base}/content`, isContent),
    leaderboard: () => call(`${base}/leaderboard`, isLeaderboard),
  }
}

/** The client for the page origin, using the global `fetch`. */
export const delvesApi: DelvesApi = {
  ...createDelvesApi(),
  follow: (id, handlers) => followDelve(id, handlers),
}

/** What a run's socket tells its page. */
export interface DelveFollowHandlers {
  /** The run as the caller sees it: on every (re)connect, then after every change. */
  view(view: DelveView): void
  /** The socket closed; it reconnects by itself, and the page reads the run once meanwhile. */
  lost(): void
}

/** Options of `followDelve`; tests pass a fake network and clock. */
export interface FollowOptions {
  createSocket?: SocketFactory
  timers?: Timers
  /** Jitter source in [0, 1); defaults to `Math.random`. */
  random?: () => number
  /** Origin of the page, such as `https://play.example`; defaults to `window.location.origin`. */
  origin?: string
}

/** The socket address that follows run `id` on `origin` (`/ws/delves/{id}`), with ws or wss to match its scheme. */
export function delveSocketUrl(id: string, origin: string): string {
  const url = new URL(`/ws/delves/${encodeURIComponent(id)}`, origin)
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:'
  return url.href
}

/**
 * Follows run `id` over its socket (docs/protocol.md, Delves socket): each frame is a `DelveView`; a closed socket
 * reports `lost` and reconnects with the game socket's backoff. Answers the function that stops following.
 */
export function followDelve(
  id: string,
  handlers: DelveFollowHandlers,
  options: FollowOptions = {},
): () => void {
  const createSocket = options.createSocket ?? browserSocket
  const timers = options.timers ?? browserTimers
  const random = options.random ?? Math.random
  const url = delveSocketUrl(id, options.origin ?? window.location.origin)
  let stopped = false
  let failures = 0
  let socket: RawSocket | null = null
  let retry: number | null = null
  const connect = () => {
    retry = null
    socket = createSocket(url, {
      open: () => undefined,
      message: (data) => {
        if (stopped || typeof data !== 'string') return
        let body: unknown = null
        try {
          body = JSON.parse(data)
        } catch {
          return
        }
        if (!isView(body)) return
        failures = 0
        handlers.view(body)
      },
      close: () => {
        socket = null
        if (stopped) return
        handlers.lost()
        failures += 1
        retry = timers.setTimeout(connect, retryDelay(failures, random()))
      },
    })
  }
  connect()
  return () => {
    stopped = true
    if (retry !== null) timers.clearTimeout(retry)
    socket?.close(1000, 'left the run')
    socket = null
  }
}
