/**
 * The deck endpoints of `mtg-server` (docs/server.md, Cards and decks): list, read, save, replace, check, import,
 * export and delete.
 * The bodies are the generated types of `mtg_protocol::rest`; every answer is checked at runtime before it is
 * returned and every failure is a `DeckError` with a stable code. Every request carries the session cookie
 * (`credentials: 'include'`), since the deck routes need a signed-in account.
 */
import type {
  CardHit,
  CheckRequest,
  Counts,
  DeckBody,
  DeckErrorCode,
  DeckLineView,
  DeckListItem,
  DeckReport,
  DeckState,
  DeckView,
  DeckZone,
  IgnoredCards,
  ImportRequest,
  ImportResult,
  ImportSource,
  ListProblem,
  Reason,
  ReadState,
  ReportCard,
  Summary,
  Support,
  UnresolvedLine,
  UnresolvedReason,
  ZoneCounts,
} from '@/protocol/generated'
import type { Fetch } from './api'
import { isCount, isFlag, isListOf, isRecord, isText } from './check'

// A `Record` over every code, so a code added in Rust (and so to the generated union) stops this file compiling.
const DECK_CODES: Record<DeckErrorCode, true> = {
  invalid_request: true,
  invalid_deck: true,
  unknown_cards: true,
  not_found: true,
  deck_limit: true,
  export_failed: true,
  unauthenticated: true,
  forbidden_origin: true,
  payload_too_large: true,
  unsupported_media_type: true,
  decks_unavailable: true,
  cards_unavailable: true,
  unavailable: true,
  internal: true,
  rate_limited: true,
}

/** True for a code the card and deck endpoints send. */
export const isDeckErrorCode = (value: string): value is DeckErrorCode => Object.hasOwn(DECK_CODES, value)

/** Every `error` code the deck endpoints send (`DeckErrorCode`). */
export const DECK_ERROR_CODES: readonly DeckErrorCode[] = Object.keys(DECK_CODES).filter(isDeckErrorCode)

/** The codes of a `DeckError`: the server's, plus `unknown`, `network` and `bad_response` raised locally. */
export type DeckFailureCode = DeckErrorCode | 'unknown' | 'network' | 'bad_response'

/** A failed deck call. `message` is the server's own sentence or ours; show `describeDeckError` instead. */
export class DeckError extends Error {
  /** What went wrong, for code to act on. */
  readonly code: DeckFailureCode
  /** The HTTP status, or 0 when no answer arrived. */
  readonly status: number

  constructor(code: DeckFailureCode, status: number, message: string) {
    super(message)
    this.name = 'DeckError'
    this.code = code
    this.status = status
  }
}

/** The kinds of file the export endpoint writes (`format` of `GET /api/decks/{id}/export`). */
export type ExportFormat = 'text' | 'arena' | 'mtgo'

/** Options for `createDecksApi`. */
export interface DecksOptions {
  /** Defaults to the global `fetch`, looked up at call time. */
  fetch?: Fetch
  /** Prefix for every path; empty means the page origin, which Vite proxies to the server in development. */
  baseUrl?: string
}

/** The deck calls; each rejects with a `DeckError`. */
export interface DecksApi {
  /** `GET /api/decks`: the account's decks, the most recently saved first. */
  list(): Promise<DeckListItem[]>
  /** `GET /api/decks/{id}`: the lines, the stored check and the report of the moment. */
  get(id: string): Promise<DeckView>
  /** `POST /api/decks`: stores a deck and runs its check. */
  create(deck: DeckBody): Promise<DeckView>
  /** `PUT /api/decks/{id}`: replaces the name, format, notes and every line, and runs the check again. */
  update(id: string, deck: DeckBody): Promise<DeckView>
  /** `POST /api/decks/check`: the report for an unsaved list or a stored deck, nothing saved; `signal` cancels it. */
  check(request: CheckRequest, signal?: AbortSignal): Promise<DeckReport>
  /** `POST /api/decks/import`: reads a pasted list into lines without saving anything. */
  importList(request: ImportRequest): Promise<ImportResult>
  /** `DELETE /api/decks/{id}`. */
  remove(id: string): Promise<void>
  /** The address of `GET /api/decks/{id}/export`; the browser saves the answer under the name the server gives it. */
  exportUrl(id: string, format?: ExportFormat): string
}

const STATES: Record<DeckState, true> = {
  verified: true,
  read: true,
  manual_play: true,
  blocked: true,
  unchecked: true,
}
const READ_STATES: Record<ReadState, true> = { pending: true, approved: true, wrong: true }
const ZONES: Record<DeckZone, true> = { main: true, sideboard: true, commander: true, companion: true }
const SOURCES: Record<ImportSource, true> = {
  text: true,
  arena: true,
  mtgo: true,
  mtgo_dek: true,
  moxfield: true,
  archidekt: true,
}
const SUPPORTS: Record<Support, true> = {
  verified: true,
  read: true,
  partial: true,
  manual: true,
  stale: true,
  unplayable: true,
}
const REASONS: Record<UnresolvedReason, true> = { unknown_card: true, ambiguous_name: true }

/** True for a deck state of the wire. */
export const isState = (value: unknown): value is DeckState => isText(value) && Object.hasOwn(STATES, value)
/** True for a deck zone of the wire. */
export const isZone = (value: unknown): value is DeckZone => isText(value) && Object.hasOwn(ZONES, value)
const isSource = (value: unknown): value is ImportSource => isText(value) && Object.hasOwn(SOURCES, value)
/** True for a support state of the wire. */
/** True for the read state of a draft. */
const isReadState = (value: unknown): value is ReadState => isText(value) && Object.hasOwn(READ_STATES, value)

export const isSupport = (value: unknown): value is Support => isText(value) && Object.hasOwn(SUPPORTS, value)
const isReason = (value: unknown): value is UnresolvedReason => isText(value) && Object.hasOwn(REASONS, value)
const isNullableText = (value: unknown): value is string | null => value === null || isText(value)

function readCounts(value: unknown): Counts | null {
  if (!isRecord(value)) return null
  const { total, verified, read, partial, manual, stale, unplayable } = value
  if (!isCount(total) || !isCount(verified) || !isCount(read) || !isCount(partial)) return null
  if (!isCount(manual) || !isCount(stale) || !isCount(unplayable)) return null
  return { total, verified, read, partial, manual, stale, unplayable }
}

/** Reads the summary of a check; null when a field is missing or of another kind. */
export function readSummary(value: unknown): Summary | null {
  if (!isRecord(value) || !isText(value.format) || !isState(value.state)) return null
  if (!isCount(value.issues) || !isCount(value.to_read) || !Array.isArray(value.zones)) return null
  const counts = readCounts(value.counts)
  if (counts === null) return null
  const zones: ZoneCounts[] = []
  for (const entry of value.zones) {
    const zoneCounts = isRecord(entry) ? readCounts(entry.counts) : null
    if (!isRecord(entry) || !isZone(entry.zone) || zoneCounts === null) return null
    zones.push({ zone: entry.zone, counts: zoneCounts })
  }
  return {
    format: value.format,
    state: value.state,
    counts,
    zones,
    issues: value.issues,
    to_read: value.to_read,
  }
}

function readListItem(item: unknown): DeckListItem | null {
  if (!isRecord(item) || !isText(item.id) || !isText(item.name) || !isText(item.format)) return null
  if (!isText(item.created_at) || !isText(item.updated_at) || !isState(item.state)) return null
  if (!isFlag(item.verified) || !isFlag(item.changed) || !isNullableText(item.checked_at)) return null
  const summary = readSummary(item.summary)
  const commanders = readEach(item.commanders, readCardHit)
  if (summary === null || commanders === null) return null
  return {
    id: item.id,
    name: item.name,
    format: item.format,
    created_at: item.created_at,
    updated_at: item.updated_at,
    state: item.state,
    verified: item.verified,
    changed: item.changed,
    checked_at: item.checked_at,
    summary,
    commanders,
  }
}

function readList(body: unknown): DeckListItem[] | null {
  if (!isRecord(body) || !Array.isArray(body.decks)) return null
  const decks: DeckListItem[] = []
  for (const item of body.decks) {
    const deck = readListItem(item)
    if (deck === null) return null
    decks.push(deck)
  }
  return decks
}

// The pages read a deck's identity and state; the lines, the stored check and the report keep the shape the generated
// types give them, which the drift test ties to the Rust types.
function readDeckView(body: unknown): DeckView | null {
  if (!isRecord(body) || !isText(body.id) || !isText(body.name) || !isText(body.format)) return null
  if (!isText(body.notes) || !isText(body.created_at) || !isText(body.updated_at)) return null
  if (!isState(body.state) || !isFlag(body.verified) || !isFlag(body.changed)) return null
  if (!isListOf(body.cards, isRecord) || !(body.check === null || isRecord(body.check))) return null
  if (!isRecord(body.report) || readSummary(body.report.summary) === null) return null
  return body as DeckView
}

/** True for a clause the game leaves to the table. */
export const isClause = (value: unknown): value is Reason =>
  isRecord(value) &&
  isCount(value.face) &&
  isText(value.text) &&
  isRecord(value.span) &&
  isCount(value.span.start) &&
  isCount(value.span.end)

const isReportCard = (value: unknown): value is ReportCard =>
  isRecord(value) &&
  isText(value.oracle_id) &&
  isText(value.name) &&
  isZone(value.zone) &&
  isCount(value.count) &&
  isSupport(value.support) &&
  (value.draft === undefined || isReadState(value.draft)) &&
  isListOf(value.reasons, isClause)

const isIssueBody = (value: unknown): boolean =>
  isRecord(value) && isText(value.code) && isText(value.message)

// The builder draws every card of the report with its mark and reasons, so these are checked line by line.
function readReport(body: unknown): DeckReport | null {
  if (!isRecord(body) || !isText(body.deck_hash) || !isText(body.coverage_hash) || !isText(body.format)) {
    return null
  }
  if (!Array.isArray(body.legality) || !body.legality.every(isIssueBody)) return null
  if (!isListOf(body.cards, isReportCard) || readSummary(body.summary) === null) return null
  return body as DeckReport
}

/** Reads a card as the card and deck endpoints send it; null when a field is missing or of another kind. */
export function readCardHit(value: unknown): CardHit | null {
  if (!isRecord(value) || !isText(value.oracle_id) || !isText(value.name) || !isText(value.mana_cost))
    return null
  if (typeof value.mana_value !== 'number' || !isText(value.type_line) || !isSupport(value.support))
    return null
  if (!Array.isArray(value.colors) || !Array.isArray(value.color_identity) || !isRecord(value.images))
    return null
  return value as CardHit
}

function readLine(item: unknown): DeckLineView | null {
  if (!isRecord(item) || !isZone(item.zone) || !isCount(item.count) || !isNullableText(item.printing_id)) {
    return null
  }
  const card = readCardHit(item.card)
  return card === null ? null : { zone: item.zone, count: item.count, printing_id: item.printing_id, card }
}

function readUnresolved(item: unknown): UnresolvedLine | null {
  if (!isRecord(item) || !isText(item.name) || !isCount(item.count) || !isZone(item.zone)) return null
  if (!isReason(item.reason) || !isText(item.message) || !Array.isArray(item.suggestions)) return null
  const suggestions: UnresolvedLine['suggestions'] = []
  for (const suggestion of item.suggestions) {
    if (!isRecord(suggestion) || !isText(suggestion.oracle_id) || !isText(suggestion.name)) return null
    suggestions.push({ oracle_id: suggestion.oracle_id, name: suggestion.name })
  }
  return {
    name: item.name,
    count: item.count,
    zone: item.zone,
    reason: item.reason,
    message: item.message,
    suggestions,
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

const readProblem = (item: unknown): ListProblem | null =>
  isRecord(item) && isCount(item.line) && isText(item.message)
    ? { line: item.line, message: item.message }
    : null

const readIgnored = (item: unknown): IgnoredCards | null =>
  isRecord(item) && isText(item.name) && isCount(item.count) ? { name: item.name, count: item.count } : null

function readImport(body: unknown): ImportResult | null {
  if (!isRecord(body) || !isSource(body.source)) return null
  const cards = readEach(body.cards, readLine)
  const unresolved = readEach(body.unresolved, readUnresolved)
  const problems = readEach(body.problems, readProblem)
  const ignored = readEach(body.ignored, readIgnored)
  if (cards === null || unresolved === null || problems === null || ignored === null) return null
  return { source: body.source, cards, unresolved, problems, ignored }
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

const GET: RequestInit = { credentials: 'include' }
const DELETE: RequestInit = { method: 'DELETE', credentials: 'include' }
const withBody = (method: 'POST' | 'PUT', body: unknown, signal?: AbortSignal): RequestInit => ({
  method,
  credentials: 'include',
  headers: { 'content-type': 'application/json' },
  body: JSON.stringify(body),
  ...(signal === undefined ? {} : { signal }),
})

/** Builds the client; call it once, or use the shared `decksApi`. */
export function createDecksApi(options: DecksOptions = {}): DecksApi {
  const base = options.baseUrl ?? ''
  const deckPath = (id: string): string => `${base}/api/decks/${encodeURIComponent(id)}`

  async function call<T>(path: string, read: (body: unknown) => T | null, init: RequestInit): Promise<T> {
    const send: Fetch = options.fetch ?? ((url, request) => globalThis.fetch(url, request))
    let response: Response
    let text: string
    try {
      response = await send(path, init)
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
    list: () => call(`${base}/api/decks`, readList, GET),
    get: (id) => call(deckPath(id), readDeckView, GET),
    create: (deck) => call(`${base}/api/decks`, readDeckView, withBody('POST', deck)),
    update: (id, deck) => call(deckPath(id), readDeckView, withBody('PUT', deck)),
    check: (request, signal) =>
      call(`${base}/api/decks/check`, readReport, withBody('POST', request, signal)),
    importList: (request) => call(`${base}/api/decks/import`, readImport, withBody('POST', request)),
    remove: async (id) => {
      await call(deckPath(id), () => true, DELETE)
    },
    exportUrl: (id, format = 'text') => `${deckPath(id)}/export?format=${format}`,
  }
}

/** The client for the page origin, using the global `fetch`. */
export const decksApi: DecksApi = createDecksApi()
