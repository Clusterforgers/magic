/**
 * The card endpoints of `mtg-server` (docs/server.md, Cards): `GET /api/cards` searches the deck cards and
 * `GET /api/cards/{oracle_id}` reads one. Both are public and read-only. The bodies are the generated types of
 * `mtg_protocol::rest`; every answer is checked at runtime before it is returned and every failure is a `CardsError`
 * with a stable code.
 */
import type { CardDetail, CardSearchPage, DeckErrorCode } from '@/protocol/generated'
import type { Fetch } from './api'
import { isCount, isListOf, isRecord, isText } from './check'
import { isDeckErrorCode, readCardHit } from './decks'

/** The codes of a `CardsError`: the server's, plus `unknown`, `network` and `bad_response` raised locally. */
export type CardsFailureCode = DeckErrorCode | 'unknown' | 'network' | 'bad_response'

/** A failed card call. `message` is the server's own sentence or ours; show `describeCardsError` instead. */
export class CardsError extends Error {
  /** What went wrong, for code to act on. */
  readonly code: CardsFailureCode
  /** The HTTP status, or 0 when no answer arrived. */
  readonly status: number

  constructor(code: CardsFailureCode, status: number, message: string) {
    super(message)
    this.name = 'CardsError'
    this.code = code
    this.status = status
  }
}

/** True when the server has no card database (503 `cards_unavailable`), which a page shows as a notice. */
export const isCardsUnavailable = (error: unknown): boolean =>
  error instanceof CardsError && error.code === 'cards_unavailable'

/** What `GET /api/cards` is asked; a blank field is no filter. */
export interface CardQuery {
  /** Free text: the name contains it, or the rules text matches it (`q`). */
  text?: string
  /** Colour identity as letters of WUBRG, or `C`: a card matches when its identity lies within them (`identity`). */
  identity?: string
  /** Words that must each appear in the type line (`type`). */
  type?: string
  /** A mana value (`3`), a range (`2-4`) or a minimum (`5+`) (`cmc`). */
  manaValue?: string
  /** A Scryfall format key: the card is legal or restricted there (`format`). */
  format?: string
  /** The page, counted from 1 (`page`). */
  page?: number
  /** Cards per page, at most 100 (`per_page`). */
  perPage?: number
}

/** Options for `createCardsApi`. */
export interface CardsApiOptions {
  /** Defaults to the global `fetch`, looked up at call time. */
  fetch?: Fetch
  /** Prefix for every path; empty means the page origin, which Vite proxies to the server in development. */
  baseUrl?: string
}

/** The card calls; each rejects with a `CardsError`. */
export interface CardsApi {
  /** `GET /api/cards`: one page of the cards that match; `signal` cancels a request that is no longer wanted. */
  search(query: CardQuery, signal?: AbortSignal): Promise<CardSearchPage>
  /** `GET /api/cards/{oracle_id}`: the card with its text, faces, legality, printings, rulings and reasons. */
  get(oracleId: string, signal?: AbortSignal): Promise<CardDetail>
}

/** The query string of a search: blank fields are left out, values are trimmed. */
export function searchParams(query: CardQuery): URLSearchParams {
  const params = new URLSearchParams()
  const put = (key: string, value: string | undefined): void => {
    const text = value?.trim() ?? ''
    if (text !== '') params.set(key, text)
  }
  put('q', query.text)
  put('identity', query.identity)
  put('type', query.type)
  put('cmc', query.manaValue)
  put('format', query.format)
  if (query.page !== undefined) params.set('page', String(query.page))
  if (query.perPage !== undefined) params.set('per_page', String(query.perPage))
  return params
}

function readPage(body: unknown): CardSearchPage | null {
  if (!isRecord(body) || !Array.isArray(body.cards)) return null
  if (!isCount(body.total) || !isCount(body.page) || !isCount(body.per_page)) return null
  const cards: CardSearchPage['cards'] = []
  for (const item of body.cards) {
    const card = readCardHit(item)
    if (card === null) return null
    cards.push(card)
  }
  return { cards, total: body.total, page: body.page, per_page: body.per_page }
}

function readDetail(body: unknown): CardDetail | null {
  if (!isRecord(body) || readCardHit(body.card) === null || !isText(body.oracle_text)) return null
  if (!isListOf(body.keywords, isText) || !isListOf(body.faces, isRecord)) return null
  if (!isRecord(body.legalities) || !Object.values(body.legalities).every(isText)) return null
  if (!isListOf(body.printings, isRecord) || !isListOf(body.rulings, isRecord)) return null
  if (!isListOf(body.reasons, isRecord)) return null
  return body as CardDetail
}

function parseJson(text: string): unknown {
  try {
    return JSON.parse(text)
  } catch {
    return undefined
  }
}

function failure(response: Response, body: unknown): CardsError {
  if (isRecord(body) && isText(body.error) && isText(body.message)) {
    return new CardsError(isDeckErrorCode(body.error) ? body.error : 'unknown', response.status, body.message)
  }
  return new CardsError(
    'bad_response',
    response.status,
    `The server answered with HTTP ${response.status} and no readable error.`,
  )
}

/** Builds the client; call it once, or use the shared `cardsApi`. */
export function createCardsApi(options: CardsApiOptions = {}): CardsApi {
  const base = options.baseUrl ?? ''

  async function call<T>(path: string, read: (body: unknown) => T | null, signal?: AbortSignal): Promise<T> {
    const send: Fetch = options.fetch ?? ((url, request) => globalThis.fetch(url, request))
    let response: Response
    let text: string
    try {
      response = await send(path, signal === undefined ? undefined : { signal })
      text = await response.text()
    } catch {
      throw new CardsError('network', 0, 'Could not reach the server. Check your connection and try again.')
    }
    const body = parseJson(text)
    if (!response.ok) throw failure(response, body)
    const value = read(body)
    if (value === null) {
      throw new CardsError(
        'bad_response',
        response.status,
        'The server sent an answer this page cannot read.',
      )
    }
    return value
  }

  return {
    search: (query, signal) => {
      const params = searchParams(query).toString()
      return call(`${base}/api/cards${params === '' ? '' : `?${params}`}`, readPage, signal)
    },
    get: (oracleId, signal) => call(`${base}/api/cards/${encodeURIComponent(oracleId)}`, readDetail, signal),
  }
}

/** The client for the page origin, using the global `fetch`. */
export const cardsApi: CardsApi = createCardsApi()
