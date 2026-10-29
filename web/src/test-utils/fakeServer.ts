import { isRecord } from '@/net/check'
import { createApi } from '@/net/api'
import type { Api, DeckInfo, Fetch } from '@/net/api'
import { reportCard, reportOf } from '@/features/decks/fakeDecks'
import { createDecksApi } from '@/net/decks'
import type { DecksApi } from '@/net/decks'
import { createPreconsApi } from '@/net/precons'
import type { PreconsApi } from '@/net/precons'
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
  /**
   * Answers the changes a seat makes: `PATCH` and `DELETE /api/games/{id}`, `POST .../leave` and
   * `POST .../seats/{n}/release`. The good answer by default (`changedReply`).
   */
  change?: (call: FetchCall) => Response | Promise<Response>
  /** Answers `GET /api/decks`, the signed-in account's stored decks; none by default. */
  myDecks?: (call: FetchCall) => Response | Promise<Response>
  /** Answers `POST /api/decks/check`; gets the parsed JSON body. A deck whose whole list runs by default. */
  check?: (body: unknown, call: FetchCall) => Response | Promise<Response>
  /** Answers `GET /api/decks/{id}`, a stored deck with its cards; a 404 by default. */
  deckView?: (id: string, call: FetchCall) => Response | Promise<Response>
  /** Answers `GET /api/precons/{code}`, a precon with its cards; a 404 by default. */
  preconView?: (code: string, call: FetchCall) => Response | Promise<Response>
  /** Answers `GET /api/precons`; none by default. */
  precons?: (call: FetchCall) => Response | Promise<Response>
  /**
   * Answers `PUT /api/games/{id}/seats/{n}/deck`; gets the parsed JSON body. By default a built-in deck is answered
   * with its facts (`demoDeckReply`) and any other id with the server's 404 `unknown_deck`.
   */
  chooseDeck?: (body: unknown, call: FetchCall) => Response | Promise<Response>
}

/** A REST server in memory, with a record of what was asked of it. */
export interface FakeServer {
  api: Api
  /** The deck endpoints on the same fake `fetch`, for the stored decks of a signed-in player. */
  decks: DecksApi
  /** The precon endpoints on the same fake `fetch`. */
  precons: PreconsApi
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

/** The reply of a real server to a good change: the visibility now in force for a `PATCH`, 204 for the rest. */
export function changedReply(call: FetchCall): Response {
  if (call.init?.method !== 'PATCH') return new Response(null, { status: 204 })
  const body: unknown = typeof call.init.body === 'string' ? JSON.parse(call.init.body) : null
  return jsonResponse(200, { visibility: isRecord(body) ? body.visibility : 'invite' })
}

/** The reply of a real server to a good choice of a built-in deck: its name, `normal` and no manual cards. */
export function demoDeckReply(body: unknown, decks: readonly DeckInfo[] = DECKS): Response {
  const asked = isRecord(body) ? body.deck : undefined
  const deck = decks.find((candidate) => candidate.id === asked)
  if (deck === undefined) {
    return jsonResponse(404, { error: 'unknown_deck', message: 'no such deck' })
  }
  return jsonResponse(200, { name: deck.name, format: 'normal', manual_cards: 0 })
}

const isChange = (path: string, method: string | undefined): boolean =>
  method === 'PATCH' ||
  method === 'DELETE' ||
  (method === 'POST' && /\/(leave|seats\/\d+\/release)$/.test(path))

/** Builds a REST client whose `fetch` is answered by `script`; unknown paths get the server's 404. */
export function startServer(script: ServerScript = {}, decks: DeckInfo[] = DECKS): FakeServer {
  const creates: unknown[] = []
  const joins: unknown[] = []
  const fake = fakeFetch((call) => {
    const path = call.url.split('?')[0] ?? call.url
    if (path === '/api/demo-decks') return script.decks?.(call) ?? jsonResponse(200, decks)
    if (path === '/api/lobbies') return script.lobbies?.(call) ?? jsonResponse(200, { games: [] })
    if (path === '/api/decks' && call.init?.method === undefined) {
      return script.myDecks?.(call) ?? jsonResponse(200, { decks: [] })
    }
    if (path === '/api/precons') return script.precons?.(call) ?? jsonResponse(200, { precons: [] })
    const viewOf = /^\/api\/(decks|precons)\/([^/]+)$/.exec(path)
    if (viewOf !== null && call.init?.method === undefined && viewOf[2] !== 'check') {
      const id = decodeURIComponent(viewOf[2] ?? '')
      const answer = viewOf[1] === 'decks' ? script.deckView?.(id, call) : script.preconView?.(id, call)
      return answer ?? jsonResponse(404, { error: 'not_found', message: 'No such deck.' })
    }
    if (path === '/api/decks/check' && call.init?.method === 'POST') {
      const body: unknown = typeof call.init.body === 'string' ? JSON.parse(call.init.body) : null
      return script.check?.(body, call) ?? jsonResponse(200, reportOf([reportCard('Forest', { count: 60 })]))
    }
    if (call.init?.method === 'PUT' && /^\/api\/games\/[^/]+\/seats\/\d+\/deck$/.test(path)) {
      const body: unknown = typeof call.init.body === 'string' ? JSON.parse(call.init.body) : null
      return script.chooseDeck?.(body, call) ?? demoDeckReply(body, decks)
    }
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
    if (path.startsWith('/api/games/') && isChange(path, call.init?.method)) {
      return script.change?.(call) ?? changedReply(call)
    }
    return jsonResponse(404, { error: 'not_found', message: 'no such resource' })
  })
  return {
    api: createApi({ fetch: fake.fetch }),
    decks: createDecksApi({ fetch: fake.fetch }),
    precons: createPreconsApi({ fetch: fake.fetch }),
    fetch: fake.fetch,
    calls: fake.calls,
    creates,
    joins,
    count: (path) => fake.calls.filter((call) => call.url === path).length,
  }
}
