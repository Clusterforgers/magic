/**
 * The account endpoints of `mtg-server` (docs/server.md, Accounts): register, sign in, sign out, who am I, the
 * signed-in user's games, changing the password and deleting the account. The bodies are the generated types of `mtg_protocol::rest`; every answer is checked at
 * runtime before it is returned and every failure is an `AccountError` with a stable code. Every request carries the
 * session cookie (`credentials: 'include'`), so a page on another origin that the server lists works as well.
 */
import type {
  AccountDeletion,
  AuthErrorCode,
  Credentials,
  MyGame,
  MyGameSeat,
  MyGameStatus,
  PasswordChange,
  UserView,
} from '@/protocol/generated'
import type { Fetch } from './api'
import { isCount, isListOf, isRecord, isText } from './check'
import { isGameResult } from './parse'

// A `Record` over every code, so a code added in Rust (and so to the generated union) stops this file compiling.
const AUTH_CODES: Record<AuthErrorCode, true> = {
  invalid_request: true,
  invalid_username: true,
  invalid_password: true,
  username_taken: true,
  registration_closed: true,
  invalid_credentials: true,
  too_many_attempts: true,
  unauthenticated: true,
  forbidden_origin: true,
  payload_too_large: true,
  unsupported_media_type: true,
  accounts_unavailable: true,
  unavailable: true,
  internal: true,
}

/** True for a code the account endpoints send. */
export const isAuthErrorCode = (value: string): value is AuthErrorCode => Object.hasOwn(AUTH_CODES, value)

/** Every `error` code the account endpoints send (`AuthErrorCode`). */
export const AUTH_ERROR_CODES: readonly AuthErrorCode[] = Object.keys(AUTH_CODES).filter(isAuthErrorCode)

/** The codes of an `AccountError`: the server's, plus `unknown`, `network` and `bad_response` raised locally. */
export type AccountErrorCode = AuthErrorCode | 'unknown' | 'network' | 'bad_response'

/** A failed account call. `message` is the server's own sentence or ours; show `describeAccountError` instead. */
export class AccountError extends Error {
  /** What went wrong, for code to act on. */
  readonly code: AccountErrorCode
  /** The HTTP status, or 0 when no answer arrived. */
  readonly status: number
  /** Seconds the server asked the client to wait (`Retry-After` of a 429), or null. */
  readonly retryAfter: number | null

  constructor(code: AccountErrorCode, status: number, message: string, retryAfter: number | null = null) {
    super(message)
    this.name = 'AccountError'
    this.code = code
    this.status = status
    this.retryAfter = retryAfter
  }
}

/** Options for `createAccountsApi`. */
export interface AccountsOptions {
  /** Defaults to the global `fetch`, looked up at call time. */
  fetch?: Fetch
  /** Prefix for every path; empty means the page origin, which Vite proxies to the server in development. */
  baseUrl?: string
}

/** The account calls; each rejects with an `AccountError`. */
export interface AccountsApi {
  /** `POST /api/auth/register`: makes the account and signs it in. */
  register(credentials: Credentials): Promise<UserView>
  /** `POST /api/auth/login`: signs in. */
  login(credentials: Credentials): Promise<UserView>
  /** `POST /api/auth/logout`: ends the session; fine without one. */
  logout(): Promise<void>
  /** `GET /api/auth/me`: the signed-in account; rejects with `unauthenticated` when there is none. */
  me(): Promise<UserView>
  /** `GET /api/me/games`: the games the user holds a seat in, as the server sorted them. */
  myGames(): Promise<MyGame[]>
  /** `DELETE /api/me/games/{id}`: takes a failed game off the user's list; rejects with a 404 when there is none. */
  removeGame(game: string): Promise<void>
  /** `POST /api/auth/password`: replaces the password; the account's other sessions end and this one stays. */
  changePassword(change: PasswordChange): Promise<void>
  /** `DELETE /api/auth/account`: deletes the account, its decks and its sessions; the cookie is cleared. */
  deleteAccount(deletion: AccountDeletion): Promise<void>
}

const STATUSES: Record<MyGameStatus, true> = { lobby: true, running: true, finished: true, failed: true }
const isStatus = (value: unknown): value is MyGameStatus => isText(value) && Object.hasOwn(STATUSES, value)

function readUser(body: unknown): UserView | null {
  if (!isRecord(body) || !isText(body.id) || !isText(body.username)) return null
  return { id: body.id, username: body.username }
}

function readSeat(item: unknown): MyGameSeat | null {
  if (!isRecord(item) || !isCount(item.seat) || !isText(item.name)) return null
  if (item.user !== null && !isText(item.user)) return null
  return { seat: item.seat, name: item.name, user: item.user }
}

function readGame(item: unknown): MyGame | null {
  if (!isRecord(item) || !isText(item.game) || !isCount(item.seat) || !isText(item.name)) return null
  if (!isStatus(item.status) || !isText(item.created_at) || !Array.isArray(item.others)) return null
  if (item.result !== null && !isGameResult(item.result)) return null
  if (item.reason !== null && !isText(item.reason)) return null
  const others: MyGameSeat[] = []
  for (const entry of item.others) {
    const seat = readSeat(entry)
    if (seat === null) return null
    others.push(seat)
  }
  return {
    game: item.game,
    seat: item.seat,
    name: item.name,
    status: item.status,
    created_at: item.created_at,
    others,
    result: item.result,
    reason: item.reason,
  }
}

function readGames(body: unknown): MyGame[] | null {
  if (!isRecord(body) || !isListOf(body.games, isRecord)) return null
  const games: MyGame[] = []
  for (const item of body.games) {
    const game = readGame(item)
    if (game === null) return null
    games.push(game)
  }
  return games
}

function parseJson(text: string): unknown {
  try {
    return JSON.parse(text)
  } catch {
    return undefined
  }
}

// `Retry-After` is whole seconds or an HTTP date; the server sends seconds, and a date is simply not used.
function waitOf(response: Response): number | null {
  const value = response.headers.get('retry-after')
  return value !== null && /^\d{1,9}$/.test(value.trim()) ? Number(value.trim()) : null
}

function failure(response: Response, body: unknown): AccountError {
  if (isRecord(body) && isText(body.error) && isText(body.message)) {
    const code = isAuthErrorCode(body.error) ? body.error : 'unknown'
    return new AccountError(code, response.status, body.message, waitOf(response))
  }
  return new AccountError(
    'bad_response',
    response.status,
    `The server answered with HTTP ${response.status} and no readable error.`,
  )
}

const send = (
  method: 'POST' | 'DELETE',
  body?: Credentials | PasswordChange | AccountDeletion,
): RequestInit => ({
  method,
  credentials: 'include',
  ...(body === undefined
    ? {}
    : { headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) }),
})

const post = (body?: Credentials | PasswordChange): RequestInit => send('POST', body)

const get: RequestInit = { credentials: 'include' }

/** Builds the client; call it once, or use the shared `accountsApi`. */
export function createAccountsApi(options: AccountsOptions = {}): AccountsApi {
  const base = options.baseUrl ?? ''

  async function call<T>(path: string, read: (body: unknown) => T | null, init: RequestInit): Promise<T> {
    const send: Fetch = options.fetch ?? ((url, request) => globalThis.fetch(url, request))
    let response: Response
    let text: string
    try {
      response = await send(`${base}${path}`, init)
      text = await response.text()
    } catch {
      throw new AccountError('network', 0, 'Could not reach the server. Check your connection and try again.')
    }
    const body = parseJson(text)
    if (!response.ok) throw failure(response, body)
    const value = read(body)
    if (value === null) {
      throw new AccountError(
        'bad_response',
        response.status,
        'The server sent an answer this page cannot read.',
      )
    }
    return value
  }

  return {
    register: (credentials) => call('/api/auth/register', readUser, post(credentials)),
    login: (credentials) => call('/api/auth/login', readUser, post(credentials)),
    logout: async () => {
      await call('/api/auth/logout', () => true, post())
    },
    me: () => call('/api/auth/me', readUser, get),
    myGames: () => call('/api/me/games', readGames, get),
    removeGame: async (game) => {
      await call(`/api/me/games/${encodeURIComponent(game)}`, () => true, send('DELETE'))
    },
    changePassword: async (change) => {
      await call('/api/auth/password', () => true, post(change))
    },
    deleteAccount: async (deletion) => {
      await call('/api/auth/account', () => true, send('DELETE', deletion))
    },
  }
}

/** The client for the page origin, using the global `fetch`. */
export const accountsApi: AccountsApi = createAccountsApi()
