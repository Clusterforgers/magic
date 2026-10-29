/**
 * The precon endpoints of `mtg-server` (docs/server.md, Precons): `GET /api/precons` lists every precon with its deck
 * check and `GET /api/precons/{code}` reads one with its cards by board. Both are public and read-only. The bodies are
 * the generated types of `mtg_protocol::rest`; every answer is checked at runtime before it is returned and every
 * failure is a `PreconError` with a stable code.
 */
import type {
  Issue,
  PreconBoard,
  PreconCardView,
  PreconErrorCode,
  PreconList,
  PreconListItem,
  PreconView,
} from '@/protocol/generated'
import type { Fetch } from './api'
import { isCount, isFlag, isListOf, isRecord, isText } from './check'
import { isClause, isState, isSupport, isZone, readCardHit, readSummary } from './decks'

// A `Record` over every code, so a code added in Rust (and so to the generated union) stops this file compiling.
const PRECON_CODES: Record<PreconErrorCode, true> = {
  not_found: true,
  rate_limited: true,
  cards_unavailable: true,
  unavailable: true,
  internal: true,
}

const isPreconErrorCode = (value: string): value is PreconErrorCode => Object.hasOwn(PRECON_CODES, value)

/** Every `error` code the precon endpoints send (`PreconErrorCode`). */
export const PRECON_ERROR_CODES: readonly PreconErrorCode[] =
  Object.keys(PRECON_CODES).filter(isPreconErrorCode)

/** The codes of a `PreconError`: the server's, plus `unknown`, `network` and `bad_response` raised locally. */
export type PreconFailureCode = PreconErrorCode | 'unknown' | 'network' | 'bad_response'

/** A failed precon call. `message` is the server's own sentence or ours; show `describePreconError` instead. */
export class PreconError extends Error {
  /** What went wrong, for code to act on. */
  readonly code: PreconFailureCode
  /** The HTTP status, or 0 when no answer arrived. */
  readonly status: number
  /** Seconds to wait before asking again, from `Retry-After` of a 429; null otherwise. */
  readonly retryAfter: number | null

  constructor(code: PreconFailureCode, status: number, message: string, retryAfter: number | null = null) {
    super(message)
    this.name = 'PreconError'
    this.code = code
    this.status = status
    this.retryAfter = retryAfter
  }
}

/** Options for `createPreconsApi`. */
export interface PreconsOptions {
  /** Defaults to the global `fetch`, looked up at call time. */
  fetch?: Fetch
  /** Prefix for every path; empty means the page origin, which Vite proxies to the server in development. */
  baseUrl?: string
}

/** The precon calls; each rejects with a `PreconError`. */
export interface PreconsApi {
  /** `GET /api/precons`: every precon, newest release first, then by name; `signal` cancels the request. */
  list(signal?: AbortSignal): Promise<PreconList>
  /** `GET /api/precons/{code}`: the precon with the issues that keep a game from taking it and its cards by board. */
  get(code: string, signal?: AbortSignal): Promise<PreconView>
}

const isNullableText = (value: unknown): value is string | null => value === null || isText(value)

/** Reads one entry of the list; null when a field is missing or of another kind. */
export function readPreconItem(item: unknown): PreconListItem | null {
  if (!isRecord(item) || !isText(item.code) || !isText(item.name) || !isText(item.set)) return null
  if (!isNullableText(item.deck_type) || !isNullableText(item.release_date)) return null
  if (!isListOf(item.commanders, isText) || !isCount(item.card_count) || !isText(item.format)) return null
  if (!isState(item.state) || !isFlag(item.verified) || !isFlag(item.playable)) return null
  const summary = readSummary(item.summary)
  if (summary === null) return null
  return {
    code: item.code,
    name: item.name,
    set: item.set,
    deck_type: item.deck_type,
    release_date: item.release_date,
    commanders: item.commanders,
    card_count: item.card_count,
    format: item.format,
    state: item.state,
    verified: item.verified,
    playable: item.playable,
    summary,
  }
}

function readEach<T>(items: unknown, read: (item: unknown) => T | null): T[] | null {
  if (!Array.isArray(items)) return null
  const out: T[] = []
  for (const item of items) {
    const value = read(item)
    if (value === null) return null
    out.push(value)
  }
  return out
}

function readList(body: unknown): PreconList | null {
  if (!isRecord(body)) return null
  const precons = readEach(body.precons, readPreconItem)
  return precons === null ? null : { precons }
}

function readCard(item: unknown): PreconCardView | null {
  if (!isRecord(item) || !isText(item.name) || !isCount(item.count) || !isNullableText(item.printing_id)) {
    return null
  }
  if (!isSupport(item.support) || !isListOf(item.reasons, isClause)) return null
  const card = item.card === null ? null : readCardHit(item.card)
  if (item.card !== null && card === null) return null
  return {
    name: item.name,
    count: item.count,
    printing_id: item.printing_id,
    card,
    support: item.support,
    reasons: item.reasons,
  }
}

function readBoard(item: unknown): PreconBoard | null {
  if (!isRecord(item) || !isZone(item.board) || !isCount(item.copies)) return null
  const cards = readEach(item.cards, readCard)
  return cards === null ? null : { board: item.board, copies: item.copies, cards }
}

const isIssue = (value: unknown): value is Issue =>
  isRecord(value) &&
  isText(value.code) &&
  isText(value.message) &&
  (value.zone === null || isZone(value.zone)) &&
  isNullableText(value.oracle_id) &&
  isNullableText(value.name)

function readView(body: unknown): PreconView | null {
  if (!isRecord(body)) return null
  const precon = readPreconItem(body.precon)
  const boards = readEach(body.boards, readBoard)
  if (precon === null || boards === null || !isListOf(body.issues, isIssue)) return null
  return { precon, issues: body.issues, boards }
}

function parseJson(text: string): unknown {
  try {
    return JSON.parse(text)
  } catch {
    return undefined
  }
}

function waitOf(response: Response): number | null {
  const seconds = Number(response.headers.get('retry-after'))
  return Number.isFinite(seconds) && seconds > 0 ? Math.ceil(seconds) : null
}

function failure(response: Response, body: unknown): PreconError {
  if (isRecord(body) && isText(body.error) && isText(body.message)) {
    const code = isPreconErrorCode(body.error) ? body.error : 'unknown'
    return new PreconError(code, response.status, body.message, waitOf(response))
  }
  return new PreconError(
    'bad_response',
    response.status,
    `The server answered with HTTP ${response.status} and no readable error.`,
  )
}

/** Builds the client; call it once, or use the shared `preconsApi`. */
export function createPreconsApi(options: PreconsOptions = {}): PreconsApi {
  const base = options.baseUrl ?? ''

  async function call<T>(path: string, read: (body: unknown) => T | null, signal?: AbortSignal): Promise<T> {
    const send: Fetch = options.fetch ?? ((url, request) => globalThis.fetch(url, request))
    let response: Response
    let text: string
    try {
      response = await send(path, signal === undefined ? undefined : { signal })
      text = await response.text()
    } catch {
      throw new PreconError('network', 0, 'Could not reach the server. Check your connection and try again.')
    }
    const body = parseJson(text)
    if (!response.ok) throw failure(response, body)
    const value = read(body)
    if (value === null) {
      throw new PreconError(
        'bad_response',
        response.status,
        'The server sent an answer this page cannot read.',
      )
    }
    return value
  }

  return {
    list: (signal) => call(`${base}/api/precons`, readList, signal),
    get: (code, signal) => call(`${base}/api/precons/${encodeURIComponent(code)}`, readView, signal),
  }
}

/** The client for the page origin, using the global `fetch`. */
export const preconsApi: PreconsApi = createPreconsApi()
