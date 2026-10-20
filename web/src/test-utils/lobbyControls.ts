import type { Visibility } from '@/protocol/generated'
import { jsonResponse } from './fakeFetch'
import { tokenOf } from './fakeServer'
import type { FakeServer, ServerScript } from './fakeServer'
import { renderGame } from './gameHarness'
import type { GameHarness } from './gameHarness'
import { lobbyMessage } from './lobbyFrames'
import type { SeatFlags } from './lobbyFrames'

/** What a lobby page in a test looks like. */
export interface LobbyScene {
  /** The seat this page plays; 0, the host, by default. */
  seat?: number
  /** What the lookup says; a function is asked at every look, to change it mid-test. */
  visibility?: Visibility | (() => Visibility)
  /** The names of the seats, in seat order; an empty name is an open seat. Ana, Bo and an open seat by default. */
  names?: string[]
  /** The lookup says the game has started (the socket still shows the lobby). */
  started?: boolean
  /** How the server answers the changes (`PATCH`, `DELETE`, leave and release); the good answer by default. */
  change?: ServerScript['change']
  /** A signed-in visitor with no seat link, who acts through the session. */
  account?: string
}

const NAMES = ['Ana', 'Bo', '']

/**
 * A lobby page whose socket is open and whose roster has been pushed: every named seat is here and nobody is ready. The
 * page is routed, so `h.router` says where Close and Leave sent it.
 */
export async function openLobby(scene: LobbyScene = {}): Promise<GameHarness> {
  const { seat = 0, visibility = 'public', names = NAMES, started = false } = scene
  const info = () =>
    jsonResponse(200, {
      started,
      seats: names.map((name, index) => ({ seat: index, name })),
      connected: names.flatMap((name, index) => (name === '' ? [] : [index])),
      ready: [],
      result: null,
      visibility: typeof visibility === 'function' ? visibility() : visibility,
    })
  const h = await renderGame({
    routed: true,
    link: scene.account === undefined ? { seat, token: tokenOf(seat) } : null,
    account: scene.account ?? null,
    script: { game: info, change: scene.change },
  })
  h.join(seat, names.length)
  h.deliver(lobbyMessage(names.map((name): SeatFlags => [name, name !== '', false])))
  await h.settle()
  return h
}

/** One change the page sent: the method, the path, the bearer token (null when it sent none) and the JSON body. */
export interface Change {
  method: string
  path: string
  authorization: string | null
  body: unknown
}

/** The changes the page has sent so far, oldest first. */
export function changesOf(server: Pick<FakeServer, 'calls'>): Change[] {
  return server.calls.flatMap((call): Change[] => {
    const method = call.init?.method
    if (method === undefined || !call.url.startsWith('/api/games/')) return []
    const headers = (call.init?.headers ?? {}) as Record<string, string>
    const text = call.init?.body
    return [
      {
        method,
        path: call.url,
        authorization: headers.authorization ?? null,
        body: typeof text === 'string' ? (JSON.parse(text) as unknown) : null,
      },
    ]
  })
}

/** A response a test lets go when it chooses, to look at the page while a request is out. */
export function held(): { answer: () => Promise<Response>; release(response: Response): void } {
  let release: (response: Response) => void = () => undefined
  const promise = new Promise<Response>((resolve) => {
    release = resolve
  })
  return { answer: () => promise, release: (response) => release(response) }
}

/** The server's answers for the failures a control must put in words. */
export const FAILURES = {
  forbidden: () => jsonResponse(403, { error: 'forbidden', message: 'host only' }),
  gone: () => jsonResponse(404, { error: 'not_found', message: 'no such game' }),
  started: () => jsonResponse(409, { error: 'started', message: 'already started' }),
  notClaimed: () => jsonResponse(409, { error: 'not_claimed', message: 'issued seat' }),
  server: () => jsonResponse(500, { error: 'internal', message: 'boom' }),
  busy: () => jsonResponse(503, { error: 'unavailable', message: 'restarting' }),
  proxy: () => new Response('<html>Bad gateway</html>', { status: 502 }),
  offline: () => Promise.reject(new TypeError('Failed to fetch')),
} as const

/** The rows of the seat list, in seat order; found even while a dialog hides the page behind it (and its names). */
export const seatItems = (): HTMLElement[] =>
  Array.from(document.querySelectorAll<HTMLElement>('ul[aria-label="Seats"] > li'))
