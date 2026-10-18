import { createAccountsApi } from '@/net/accounts'
import type { AccountsApi } from '@/net/accounts'
import type { Fetch } from '@/net/api'
import { isRecord } from '@/net/check'
import type { AuthErrorCode, MyGame } from '@/protocol/generated'
import { passwordProblem, usernameProblem } from '@/features/auth/rules'
import { fakeFetch, jsonResponse } from './fakeFetch'
import type { FetchCall } from './fakeFetch'

/** What the fake account server does beyond the usual; leave a field out for the usual good behaviour. */
export interface AccountsScript {
  /** Accounts that exist, as username to password. */
  users?: Record<string, string>
  /** The username already signed in when the page opens (the cookie the browser holds). */
  signedIn?: string
  /** What `GET /api/me/games` lists for the signed-in user. */
  games?: MyGame[]
  /** `register` answers 403 `registration_closed`. */
  closed?: boolean
  /** The server has no database: every account call is 503 `accounts_unavailable`. */
  unavailable?: boolean
  /** After this many failed logins the server answers 429 with `Retry-After` (900 seconds). */
  throttleAfter?: number
  /** Replaces the answer of one endpoint (`me`, `register`, `login`, `logout`, `games`). */
  override?: Partial<
    Record<
      'me' | 'register' | 'login' | 'logout' | 'games',
      (call: FetchCall) => Response | Promise<Response>
    >
  >
}

/** An account server in memory, with a record of what it was asked. */
export interface AccountsServer {
  api: AccountsApi
  fetch: Fetch
  calls: FetchCall[]
  /** The bodies of every register and login request, parsed, in order. */
  credentials: unknown[]
  /** The username signed in right now (what the cookie stands for), or null. */
  readonly signedIn: string | null
  /** The accounts that exist, by username as registered. */
  readonly users: ReadonlyMap<string, string>
  /** Changes what `GET /api/me/games` lists. */
  setGames(games: MyGame[]): void
}

/** The error body of `mtg-server`. */
export const authError = (
  status: number,
  error: AuthErrorCode,
  message = 'refused',
  headers?: HeadersInit,
): Response =>
  new Response(JSON.stringify({ error, message }), {
    status,
    headers: { 'content-type': 'application/json', ...headers },
  })

/** A game of the signed-in user's list, with the fields a test does not care about filled in. */
export function listed(over: Partial<MyGame> = {}): MyGame {
  return {
    game: '8f2c1a52-31de-4c77-9a35-0d1d3f6a7b10',
    seat: 0,
    name: 'Ada',
    status: 'running',
    created_at: '2026-10-18T10:00:00Z',
    others: [{ seat: 1, name: 'Bo', user: null }],
    result: null,
    ...over,
  }
}

/** The user id the fake server gives an account. */
export const idOf = (username: string): string => `id-${username.toLowerCase()}`

/** Builds a fake of the account endpoints; `api` is the real client over it, so parsing is exercised too. */
export function startAccounts(script: AccountsScript = {}): AccountsServer {
  const users = new Map<string, string>(Object.entries(script.users ?? {}))
  let signedIn: string | null = script.signedIn ?? null
  let games = script.games ?? []
  let failures = 0
  const credentials: unknown[] = []

  const find = (name: string): string | undefined =>
    [...users.keys()].find((known) => known.toLowerCase() === name.toLowerCase())
  const view = (name: string) => ({ id: idOf(name), username: name })
  const unavailable = () => authError(503, 'accounts_unavailable', 'this server has no accounts')
  const body = (call: FetchCall): unknown =>
    typeof call.init?.body === 'string' ? JSON.parse(call.init.body) : null

  function register(call: FetchCall): Response {
    const sent = body(call)
    credentials.push(sent)
    if (script.closed === true) return authError(403, 'registration_closed')
    if (!isRecord(sent) || typeof sent.username !== 'string' || typeof sent.password !== 'string') {
      return authError(400, 'invalid_request')
    }
    if (usernameProblem(sent.username) !== null) return authError(400, 'invalid_username')
    if (passwordProblem(sent.password) !== null) return authError(400, 'invalid_password')
    if (find(sent.username) !== undefined) return authError(409, 'username_taken')
    users.set(sent.username, sent.password)
    signedIn = sent.username
    return jsonResponse(201, view(sent.username))
  }

  function login(call: FetchCall): Response {
    const sent = body(call)
    credentials.push(sent)
    if (script.throttleAfter !== undefined && failures >= script.throttleAfter) {
      return authError(429, 'too_many_attempts', 'slow down', { 'retry-after': '900' })
    }
    const known = isRecord(sent) && typeof sent.username === 'string' ? find(sent.username) : undefined
    if (known === undefined || !isRecord(sent) || users.get(known) !== sent.password) {
      failures += 1
      return authError(401, 'invalid_credentials', 'wrong username or password')
    }
    signedIn = known
    return jsonResponse(200, view(known))
  }

  function logout(): Response {
    signedIn = null
    return new Response(null, { status: 204 })
  }

  const fake = fakeFetch((call) => {
    const path = call.url.split('?')[0] ?? call.url
    if (script.unavailable === true) return unavailable()
    switch (path) {
      case '/api/auth/register':
        return script.override?.register?.(call) ?? register(call)
      case '/api/auth/login':
        return script.override?.login?.(call) ?? login(call)
      case '/api/auth/logout':
        return script.override?.logout?.(call) ?? logout()
      case '/api/auth/me':
        return (
          script.override?.me?.(call) ??
          (signedIn === null ? authError(401, 'unauthenticated') : jsonResponse(200, view(signedIn)))
        )
      case '/api/me/games':
        return (
          script.override?.games?.(call) ??
          (signedIn === null ? authError(401, 'unauthenticated') : jsonResponse(200, { games }))
        )
      default:
        return jsonResponse(404, { error: 'not_found', message: 'no such resource' })
    }
  })

  return {
    api: createAccountsApi({ fetch: fake.fetch }),
    fetch: fake.fetch,
    calls: fake.calls,
    credentials,
    get signedIn() {
      return signedIn
    },
    users,
    setGames(next) {
      games = next
    },
  }
}

/** One `fetch` for a page that talks to both servers: account paths go to `accounts`, the rest to `rest`. */
export function joinFetch(accounts: Fetch, rest: Fetch): Fetch {
  return (url, init) =>
    url.startsWith('/api/auth/') || url.startsWith('/api/me/') ? accounts(url, init) : rest(url, init)
}
