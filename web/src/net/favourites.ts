/**
 * The favourite-precon endpoints of `mtg-server` (docs/server.md, Favourite precons): list the account's stars, star a
 * precon and take the star back. A favourite is a pointer to the precon, never a copy. The list is the generated
 * `FavouritePrecons`; every answer is checked at runtime and every failure is a `DeckError` with a stable code, since
 * the endpoints share the deck endpoints' codes. Every request carries the session cookie (`credentials: 'include'`).
 */
import type { FavouritePrecons } from '@/protocol/generated'
import type { Fetch } from './api'
import { isListOf, isRecord, isText } from './check'
import { DeckError, isDeckErrorCode } from './decks'

/** Options for `createFavouritesApi`. */
export interface FavouritesOptions {
  /** Defaults to the global `fetch`, looked up at call time. */
  fetch?: Fetch
  /** Prefix for every path; empty means the page origin, which Vite proxies to the server in development. */
  baseUrl?: string
}

/** The favourite calls; each rejects with a `DeckError`. */
export interface FavouritesApi {
  /** `GET /api/favourites/precons`: the codes of the account's starred precons, the newest star first. */
  list(): Promise<string[]>
  /** `PUT /api/favourites/precons/{code}`: stars the precon; resolves when it was starred already. */
  star(code: string): Promise<void>
  /** `DELETE /api/favourites/precons/{code}`: takes the star back; resolves when there was none. */
  unstar(code: string): Promise<void>
}

function readList(body: unknown): string[] | null {
  if (!isRecord(body) || !isListOf(body.codes, isText)) return null
  const answer: FavouritePrecons = { codes: body.codes }
  return answer.codes
}

function parseJson(text: string): unknown {
  try {
    return JSON.parse(text)
  } catch {
    return undefined
  }
}

function failure(response: Response, body: unknown): DeckError {
  if (isRecord(body) && isText(body.error) && isText(body.message)) {
    return new DeckError(isDeckErrorCode(body.error) ? body.error : 'unknown', response.status, body.message)
  }
  return new DeckError(
    'bad_response',
    response.status,
    `The server answered with HTTP ${response.status} and no readable error.`,
  )
}

/** Builds the client; call it once, or use the shared `favouritesApi`. */
export function createFavouritesApi(options: FavouritesOptions = {}): FavouritesApi {
  const base = options.baseUrl ?? ''
  const path = (code: string): string => `${base}/api/favourites/precons/${encodeURIComponent(code)}`

  async function call<T>(url: string, read: (body: unknown) => T | null, init: RequestInit): Promise<T> {
    const send: Fetch = options.fetch ?? ((target, request) => globalThis.fetch(target, request))
    let response: Response
    let text: string
    try {
      response = await send(url, { credentials: 'include', ...init })
      text = await response.text()
    } catch {
      throw new DeckError('network', 0, 'Could not reach the server. Check your connection and try again.')
    }
    const body = parseJson(text)
    if (!response.ok) throw failure(response, body)
    const value = read(body)
    if (value === null) {
      throw new DeckError('bad_response', response.status, 'The server sent an answer this page cannot read.')
    }
    return value
  }

  return {
    list: () => call(`${base}/api/favourites/precons`, readList, {}),
    star: async (code) => {
      await call(path(code), () => true, { method: 'PUT' })
    },
    unstar: async (code) => {
      await call(path(code), () => true, { method: 'DELETE' })
    },
  }
}

/** The client for the page origin, using the global `fetch`. */
export const favouritesApi: FavouritesApi = createFavouritesApi()
