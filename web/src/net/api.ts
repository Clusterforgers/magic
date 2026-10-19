/**
 * The REST client for `mtg-server` (docs/server.md). The bodies are not generated, so their types are written here
 * and every response is checked at runtime before it is returned.
 */
import type { Color, GameResult, PlayerId } from '@/protocol/generated'
import { isCount, isFlag, isListOf, isRecord, isText } from './check'
import { isGameResult } from './parse'

/** One seat of a new game: the name shown to others and a deck id from `listDecks`. */
export interface SeatRequest {
  name: string
  deck: string
}

/** The body of `createGame`; the server only plays "normal" for now. */
export interface CreateGameRequest {
  format: 'normal'
  seats: SeatRequest[]
}

/** A seat's secret token, shown once at creation; it opens that seat's WebSocket. */
export interface CreatedSeat {
  seat: PlayerId
  token: string
}

/** A new game and the token of each seat. */
export interface CreatedGame {
  game: string
  seats: CreatedSeat[]
}

/** The public facts about a game; never a secret. */
export interface GameInfo {
  started: boolean
  seats: { seat: PlayerId; name: string }[]
  connected: PlayerId[]
  /** The seats that pressed Ready and did not take it back; every seat once the game runs. */
  ready: PlayerId[]
  result: GameResult | null
}

/** A built-in deck with its card list. */
export interface DeckInfo {
  id: string
  name: string
  color: Color
  size: number
  cards: { name: string; count: number }[]
}

/** The `error` codes `mtg-server` sends; `unknown`, `network` and `bad_response` are raised locally. */
export type ApiErrorCode =
  | 'invalid_request'
  | 'unsupported_format'
  | 'unknown_deck'
  | 'forbidden_origin'
  | 'not_found'
  | 'method_not_allowed'
  | 'payload_too_large'
  | 'unsupported_media_type'
  | 'internal'
  | 'server_full'
  | 'unavailable'
  | 'unknown'
  | 'network'
  | 'bad_response'

type ServerCode = Exclude<ApiErrorCode, 'unknown' | 'network' | 'bad_response'>

const SERVER_CODES: Record<ServerCode, true> = {
  invalid_request: true,
  unsupported_format: true,
  unknown_deck: true,
  forbidden_origin: true,
  not_found: true,
  method_not_allowed: true,
  payload_too_large: true,
  unsupported_media_type: true,
  internal: true,
  server_full: true,
  unavailable: true,
}

/** The codes `mtg-server` can send, as listed in `crates/mtg-server/src/api.rs`. */
export const SERVER_ERROR_CODES: readonly string[] = Object.keys(SERVER_CODES)

const COLORS: Record<Color, true> = { White: true, Blue: true, Black: true, Red: true, Green: true }

/** A failed REST call. `message` is fit to show a player. */
export class ApiError extends Error {
  /** What went wrong, for code to act on. */
  readonly code: ApiErrorCode
  /** The HTTP status, or 0 when no answer arrived. */
  readonly status: number

  constructor(code: ApiErrorCode, status: number, message: string) {
    super(message)
    this.name = 'ApiError'
    this.code = code
    this.status = status
  }
}

/** The `fetch` the client calls; the global one fits, and so does a fake. */
export type Fetch = (url: string, init?: RequestInit) => Promise<Response>

/** Options for `createApi`. */
export interface ApiOptions {
  /** Defaults to the global `fetch`, looked up at call time. */
  fetch?: Fetch
  /** Prefix for every path; empty means the page origin, which Vite proxies to the server in development. */
  baseUrl?: string
}

/** The three REST calls; each rejects with an `ApiError`. */
export interface Api {
  /** `POST /api/games`: makes a game and returns each seat's token. */
  createGame(request: CreateGameRequest): Promise<CreatedGame>
  /** `GET /api/games/{id}`: public facts about a game. */
  getGame(id: string): Promise<GameInfo>
  /** `GET /api/demo-decks`: the built-in decks. */
  listDecks(): Promise<DeckInfo[]>
}

const isServerCode = (value: string): value is ServerCode => Object.hasOwn(SERVER_CODES, value)
const isColor = (value: unknown): value is Color => isText(value) && Object.hasOwn(COLORS, value)

function parseJson(text: string): unknown {
  try {
    return JSON.parse(text)
  } catch {
    return undefined
  }
}

function failure(status: number, body: unknown): ApiError {
  if (isRecord(body) && isText(body.error) && isText(body.message)) {
    return new ApiError(isServerCode(body.error) ? body.error : 'unknown', status, body.message)
  }
  return new ApiError(
    'bad_response',
    status,
    `The server answered with HTTP ${status} and no readable error.`,
  )
}

const unreadable = (status: number): ApiError =>
  new ApiError('bad_response', status, 'The server sent an answer this page cannot read.')

function readCreated(body: unknown): CreatedGame | null {
  if (!isRecord(body) || !isText(body.game) || !Array.isArray(body.seats)) return null
  const seats: CreatedSeat[] = []
  for (const item of body.seats) {
    if (!isRecord(item) || !isCount(item.seat) || !isText(item.token)) return null
    seats.push({ seat: item.seat, token: item.token })
  }
  return { game: body.game, seats }
}

function readGame(body: unknown): GameInfo | null {
  if (!isRecord(body) || !isFlag(body.started) || !Array.isArray(body.seats)) return null
  if (!isListOf(body.connected, isCount)) return null
  // A server from before the lobby showed readiness sends no `ready`: nobody is known to be ready then.
  const ready = body.ready === undefined ? [] : body.ready
  if (!isListOf(ready, isCount)) return null
  const result = body.result === null || isGameResult(body.result) ? body.result : undefined
  if (result === undefined) return null
  const seats: GameInfo['seats'] = []
  for (const item of body.seats) {
    if (!isRecord(item) || !isCount(item.seat) || !isText(item.name)) return null
    seats.push({ seat: item.seat, name: item.name })
  }
  return { started: body.started, seats, connected: body.connected, ready, result }
}

function readDeck(item: unknown): DeckInfo | null {
  if (!isRecord(item) || !isText(item.id) || !isText(item.name) || !isColor(item.color)) return null
  if (!isCount(item.size) || !Array.isArray(item.cards)) return null
  const cards: DeckInfo['cards'] = []
  for (const line of item.cards) {
    if (!isRecord(line) || !isText(line.name) || !isCount(line.count)) return null
    cards.push({ name: line.name, count: line.count })
  }
  return { id: item.id, name: item.name, color: item.color, size: item.size, cards }
}

function readDecks(body: unknown): DeckInfo[] | null {
  if (!Array.isArray(body)) return null
  const decks: DeckInfo[] = []
  for (const item of body) {
    const deck = readDeck(item)
    if (deck === null) return null
    decks.push(deck)
  }
  return decks
}

/** Builds the client; call it once, or use the shared `api`. */
export function createApi(options: ApiOptions = {}): Api {
  const base = options.baseUrl ?? ''

  async function call<T>(path: string, read: (body: unknown) => T | null, init?: RequestInit): Promise<T> {
    const send: Fetch = options.fetch ?? ((url, request) => globalThis.fetch(url, request))
    let response: Response
    let text: string
    try {
      response = await send(`${base}${path}`, init)
      text = await response.text()
    } catch {
      throw new ApiError('network', 0, 'Could not reach the server. Check your connection and try again.')
    }
    const body = parseJson(text)
    if (!response.ok) throw failure(response.status, body)
    const value = read(body)
    if (value === null) throw unreadable(response.status)
    return value
  }

  return {
    createGame: (request) =>
      call('/api/games', readCreated, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(request),
        // The session cookie is what binds the creator's seat to their account.
        credentials: 'include',
      }),
    getGame: (id) => call(`/api/games/${encodeURIComponent(id)}`, readGame),
    listDecks: () => call('/api/demo-decks', readDecks),
  }
}

/** The client for the page origin, using the global `fetch`. */
export const api: Api = createApi()
