/**
 * The REST client for `mtg-server` (docs/server.md). The bodies of creating a game and of the demo decks are written
 * here; the game browser's and the game lookup's are generated (`mtg_protocol::rest`) and read in `parseLobbies.ts`.
 * Every response is checked at runtime before it is returned.
 */
import type {
  Color,
  GameInfo,
  HouseRules,
  Joined,
  LobbyGame,
  PlayerId,
  SeatDeck,
  Visibility,
} from '@/protocol/generated'
import { isCount, isRecord, isText } from './check'
import { LobbyError, lobbyFailure } from './lobbyErrors'
import { readGameInfo, readJoined, readLobbyList, readSeatDeck, readVisibility } from './parseLobbies'

/** One seat of a new game: the name shown to others and, for a seat that starts with one, a deck id from `listDecks`. */
export interface SeatRequest {
  /** Not read for an open seat (one past `held`): it is named by whoever takes it. */
  name: string
  /** The deck the seat starts with; left out, the seat has none until its holder chooses one in the lobby. */
  deck?: string
}

/** The body of `createGame`: a Normal game, or a Commander game (2 to 6 seats) for a Commander precon. */
export interface CreateGameRequest {
  format: 'normal' | 'commander'
  seats: SeatRequest[]
  /** Who may find and join the game; invite-only when left out. */
  visibility?: Visibility
  /** How many seats from seat 0 get a token (1 to the seat count); the rest are open. Every seat when left out. */
  held?: number
  /** What differs from the format's own rules ([`HouseRules`], docs/engine.md); left out for none. */
  house_rules?: HouseRules
}

/** A seat's secret token, shown once at creation; it opens that seat's WebSocket. */
export interface CreatedSeat {
  seat: PlayerId
  token: string
}

/** A new game, the token of each seat that is held and, from a server that makes them, the join code. */
export interface CreatedGame {
  game: string
  /** The join code, shown once; absent from a server that predates join codes. */
  code?: string
  seats: CreatedSeat[]
}

/** The seat a request speaks for: its token, or nothing to rely on the session cookie (a signed-in user's own seat). */
export interface SeatCredential {
  token?: string
}

/** What a visitor says about themselves when they join: a guest must give a name, a signed-in user need not. */
export interface JoinOptions {
  name?: string
  /** The deck the seat plays, when one is already chosen; left out, the player chooses in the lobby. */
  deck?: string
}

/**
 * The lobby list as of one answer. `fetchedAt` (ms, the client's clock) is when the rows arrived, so a row's age now is
 * `age_secs` plus the time since then; a `304` keeps the listing, and its `fetchedAt`, as it was.
 */
export interface LobbyListing {
  games: LobbyGame[]
  /** The answer's `ETag`, sent back as `If-None-Match`; null when the server sent none. */
  etag: string | null
  fetchedAt: number
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
  /** The clock `listLobbies` stamps a listing with, in milliseconds; defaults to `Date.now`. */
  now?: () => number
}

/** The REST calls. The first three reject with an `ApiError`, the game browser's and the host's with a `LobbyError`. */
export interface Api {
  /** `POST /api/games`: makes a game and returns each held seat's token and the join code. */
  createGame(request: CreateGameRequest): Promise<CreatedGame>
  /** `GET /api/games/{id}`: the public facts about a game, its visibility included; never a token or a code. */
  getGame(id: string): Promise<GameInfo>
  /** `GET /api/demo-decks`: the built-in decks. */
  listDecks(): Promise<DeckInfo[]>
  /**
   * `GET /api/lobbies`: the public games still in their lobby. Pass the listing you hold and the request carries its
   * `ETag`; when nothing changed the server answers 304 and the result is `previous` itself, so `result === previous`
   * says "no change" and costs a caller no new rows.
   */
  listLobbies(previous?: LobbyListing): Promise<LobbyListing>
  /** `POST /api/games/{id}/join`: takes the first open seat of a public game. */
  joinGame(game: string, join?: JoinOptions): Promise<Joined>
  /** `POST /api/join`: takes the first open seat of the game a join code names. */
  joinByCode(code: string, join?: JoinOptions): Promise<Joined>
  /** `POST /api/games/{id}/leave`: the holder of a claimed seat gives it back before the game starts. */
  leaveGame(game: string, as?: SeatCredential): Promise<void>
  /** `POST /api/games/{id}/seats/{n}/release`: the host removes the player in seat `n` before the game starts. */
  releaseSeat(game: string, seat: number, as?: SeatCredential): Promise<void>
  /** `PATCH /api/games/{id}`: the host lists the game or hides it again; resolves with the visibility now in force. */
  setVisibility(game: string, visibility: Visibility, as?: SeatCredential): Promise<Visibility>
  /** `DELETE /api/games/{id}`: the host closes a game that is still in its lobby; every socket of it ends with 4002. */
  closeGame(game: string, as?: SeatCredential): Promise<void>
  /**
   * `PUT /api/games/{id}/seats/{n}/deck`: the holder of seat `n` chooses the deck it plays (a demo deck's id, or the id
   * of a stored deck of the signed-in account) until the seat is ready; resolves with what the table now sees of it. A
   * refused deck rejects with a `LobbyError` of code `deck_blocked` that carries the issues.
   */
  chooseDeck(game: string, seat: number, deck: string, as?: SeatCredential): Promise<SeatDeck>
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
  // A server from before join codes sends none; one that sends a code sends text.
  if (body.code !== undefined && !isText(body.code)) return null
  const seats: CreatedSeat[] = []
  for (const item of body.seats) {
    if (!isRecord(item) || !isCount(item.seat) || !isText(item.token)) return null
    seats.push({ seat: item.seat, token: item.token })
  }
  return body.code === undefined ? { game: body.game, seats } : { game: body.game, code: body.code, seats }
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

const JSON_HEADERS = { 'content-type': 'application/json' }

// A seat token goes into a header, so it must be plain visible characters; anything else cannot be a token.
function bearer(as: SeatCredential): Record<string, string> {
  if (as.token === undefined) return {}
  if (!/^[\x21-\x7e]+$/.test(as.token)) {
    throw new LobbyError('invalid_request', 0, 'That seat link has a token the server cannot accept.')
  }
  return { authorization: `Bearer ${as.token}` }
}

const lobbyUnreadable = (status: number): LobbyError =>
  new LobbyError('bad_response', status, 'The server sent an answer this page cannot read.')

const joinBody = (join: JoinOptions): { name?: string; deck?: string } => ({
  ...(join.name === undefined ? {} : { name: join.name }),
  ...(join.deck === undefined ? {} : { deck: join.deck }),
})

/** A game browser request that changes something, with the session cookie and, if there is one, the seat's token. */
const change = (method: string, as: SeatCredential, body?: unknown): RequestInit => ({
  method,
  credentials: 'include',
  headers: body === undefined ? bearer(as) : { ...JSON_HEADERS, ...bearer(as) },
  ...(body === undefined ? {} : { body: JSON.stringify(body) }),
})

/** Builds the client; call it once, or use the shared `api`. */
export function createApi(options: ApiOptions = {}): Api {
  const base = options.baseUrl ?? ''
  const now = options.now ?? Date.now

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

  async function lobbySend(path: string, init: RequestInit): Promise<{ response: Response; body: unknown }> {
    const send: Fetch = options.fetch ?? ((url, request) => globalThis.fetch(url, request))
    try {
      const response = await send(`${base}${path}`, init)
      return { response, body: parseJson(await response.text()) }
    } catch {
      throw new LobbyError('network', 0, 'Could not reach the server. Check your connection and try again.')
    }
  }

  // The answer must be the documented success status and must read, else it is not what the server sends.
  async function lobbyCall<T>(
    path: string,
    read: (body: unknown) => T | null,
    init: RequestInit,
    success = 200,
  ): Promise<T> {
    const { response, body } = await lobbySend(path, init)
    if (!response.ok) throw lobbyFailure(response, body)
    const value = response.status === success ? read(body) : null
    if (value === null) throw lobbyUnreadable(response.status)
    return value
  }

  const game = (id: string): string => `/api/games/${encodeURIComponent(id)}`

  return {
    createGame: (request) =>
      call('/api/games', readCreated, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(request),
        // The session cookie is what binds the creator's seat to their account.
        credentials: 'include',
      }),
    getGame: (id) => call(game(id), readGameInfo),
    listDecks: () => call('/api/demo-decks', readDecks),
    listLobbies: async (previous) => {
      const known = previous?.etag ?? null
      const { response, body } = await lobbySend(
        '/api/lobbies',
        known === null ? {} : { headers: { 'if-none-match': known } },
      )
      if (response.status === 304) {
        // A 304 answers a conditional request, so one that was not is not an answer this client asked for.
        if (previous === undefined || known === null) throw lobbyUnreadable(304)
        return previous
      }
      if (!response.ok) throw lobbyFailure(response, body)
      const games = readLobbyList(body)
      if (games === null) throw lobbyUnreadable(response.status)
      return { games, etag: response.headers.get('etag'), fetchedAt: now() }
    },
    joinGame: (id, join = {}) =>
      lobbyCall(`${game(id)}/join`, readJoined, change('POST', {}, joinBody(join))),
    joinByCode: (code, join = {}) =>
      lobbyCall('/api/join', readJoined, change('POST', {}, { code, ...joinBody(join) })),
    leaveGame: async (id, as = {}) => {
      await lobbyCall(`${game(id)}/leave`, () => true, change('POST', as), 204)
    },
    releaseSeat: async (id, seat, as = {}) => {
      if (!isCount(seat)) throw new LobbyError('invalid_request', 0, 'That is not a seat number.')
      await lobbyCall(`${game(id)}/seats/${seat}/release`, () => true, change('POST', as), 204)
    },
    // `async` so that a token that cannot go into a header rejects like every other failure.
    setVisibility: async (id, visibility, as = {}) =>
      lobbyCall(game(id), readVisibility, change('PATCH', as, { visibility })),
    closeGame: async (id, as = {}) => {
      await lobbyCall(game(id), () => true, change('DELETE', as), 204)
    },
    chooseDeck: async (id, seat, deck, as = {}) => {
      if (!isCount(seat)) throw new LobbyError('invalid_request', 0, 'That is not a seat number.')
      return lobbyCall(`${game(id)}/seats/${seat}/deck`, readSeatDeck, change('PUT', as, { deck }))
    },
  }
}

/** The client for the page origin, using the global `fetch`. */
export const api: Api = createApi()
