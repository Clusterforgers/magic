import { isRecord } from '@/net/check'
import { createApi } from '@/net/api'
import type { Api, DeckInfo, Fetch } from '@/net/api'
import { fakeFetch, jsonResponse } from './fakeFetch'
import type { FetchCall } from './fakeFetch'
import { DECKS } from './decks'

/** The game id the fake server hands out. */
export const MADE_GAME = '8f2c1a52-31de-4c77-9a35-0d1d3f6a7b10'

/** The token the fake server gives `seat`: 64 hex digits like the real ones, and different for every seat. */
export const tokenOf = (seat: number): string => String(seat).padStart(2, '0').repeat(32)

/** What the fake server does for each route; leave one out for the usual good answer. */
export interface ServerScript {
  decks?: (call: FetchCall) => Response | Promise<Response>
  /** Answers `POST /api/games`; gets the parsed JSON body. */
  create?: (body: unknown, call: FetchCall) => Response | Promise<Response>
  /** Answers `GET /api/games/{id}`. */
  game?: (id: string, call: FetchCall) => Response | Promise<Response>
  /** Answers `GET /api/lobbies`; an empty list by default. */
  lobbies?: (call: FetchCall) => Response | Promise<Response>
  /** Answers `POST /api/join`; gets the parsed JSON body. Seat 1 of `MADE_GAME` by default. */
  join?: (body: unknown, call: FetchCall) => Response | Promise<Response>
}

/** A REST server in memory, with a record of what was asked of it. */
export interface FakeServer {
  api: Api
  /** The `fetch` behind `api`, for tests that stub the global one. */
  fetch: Fetch
  calls: FetchCall[]
  /** The parsed bodies of every `POST /api/games`. */
  creates: unknown[]
  /** The parsed bodies of every `POST /api/join`. */
  joins: unknown[]
  /** How many times each path was asked for. */
  count(path: string): number
}

/** The join code the fake server hands out. */
export const MADE_CODE = 'K7M29QXA'

/**
 * The reply a real server gives to a good `POST /api/games`: a game, its join code and a token for each seat that is
 * held (every seat unless the request says `held`).
 */
export function createdReply(body: unknown): Response {
  const seats = isRecord(body) && Array.isArray(body.seats) ? body.seats : []
  const held = isRecord(body) && typeof body.held === 'number' ? body.held : seats.length
  return jsonResponse(201, {
    game: MADE_GAME,
    code: MADE_CODE,
    seats: seats.slice(0, held).map((_, seat) => ({ seat, token: tokenOf(seat) })),
  })
}

/** Builds a REST client whose `fetch` is answered by `script`; unknown paths get the server's 404. */
export function startServer(script: ServerScript = {}, decks: DeckInfo[] = DECKS): FakeServer {
  const creates: unknown[] = []
  const joins: unknown[] = []
  const fake = fakeFetch((call) => {
    const path = call.url.split('?')[0] ?? call.url
    if (path === '/api/demo-decks') return script.decks?.(call) ?? jsonResponse(200, decks)
    if (path === '/api/lobbies') return script.lobbies?.(call) ?? jsonResponse(200, { games: [] })
    if (path === '/api/games' && call.init?.method === 'POST') {
      const body: unknown = typeof call.init.body === 'string' ? JSON.parse(call.init.body) : null
      creates.push(body)
      return script.create?.(body, call) ?? createdReply(body)
    }
    if (path === '/api/join' && call.init?.method === 'POST') {
      const body: unknown = typeof call.init.body === 'string' ? JSON.parse(call.init.body) : null
      joins.push(body)
      return script.join?.(body, call) ?? jsonResponse(200, { game: MADE_GAME, seat: 1, token: tokenOf(1) })
    }
    if (path.startsWith('/api/games/') && call.init?.method === undefined) {
      const id = decodeURIComponent(path.slice('/api/games/'.length))
      return script.game?.(id, call) ?? jsonResponse(404, { error: 'not_found', message: 'no such resource' })
    }
    return jsonResponse(404, { error: 'not_found', message: 'no such resource' })
  })
  return {
    api: createApi({ fetch: fake.fetch }),
    fetch: fake.fetch,
    calls: fake.calls,
    creates,
    joins,
    count: (path) => fake.calls.filter((call) => call.url === path).length,
  }
}
