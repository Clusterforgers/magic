/**
 * The read endpoints of `mtg-server` (docs/server.md, Reads): what a deck has left to read, one card's draft with its
 * read sheet, and a verdict on it. The bodies are the generated types of `mtg_protocol::rest`; every answer is checked
 * at runtime and every failure is a `DeckError` with a stable code, since the endpoints share the deck endpoints'
 * codes (`stale_read` when the draft changed since the page loaded it). Every request carries the session cookie.
 */
import type { ReadBody, ReadCard, ReadQueue, ReadRow, ReadState, ReadVerdict } from '@/protocol/generated'
import type { ReadTarget } from '@/lib/toRead'
import type { Fetch } from './api'
import { isCount, isFlag, isListOf, isRecord, isText } from './check'
import { DeckError, isDeckErrorCode } from './decks'

/** Options for `createReadsApi`. */
export interface ReadsOptions {
  /** Defaults to the global `fetch`, looked up at call time. */
  fetch?: Fetch
  /** Prefix for every path; empty means the page origin, which Vite proxies to the server in development. */
  baseUrl?: string
}

/** The read calls; each rejects with a `DeckError`. */
export interface ReadsApi {
  /** `GET /api/reads?deck=` or `?precon=`: what the account has left to read in that deck, pending drafts first. */
  queue(target: ReadTarget): Promise<ReadQueue>
  /** `GET /api/reads/{oracle_id}`: the card's current draft with its read sheet and the account's verdict. */
  get(oracleId: string): Promise<ReadCard>
  /** `PUT /api/reads/{oracle_id}`: the account's verdict on the draft with `read_hash`; answers the card after it. */
  judge(oracleId: string, body: ReadBody): Promise<ReadCard>
}

const STATES: readonly ReadState[] = ['pending', 'approved', 'wrong']
const VERDICTS: readonly ReadVerdict[] = ['correct', 'wrong']

const isState = (value: unknown): value is ReadState => STATES.includes(value as ReadState)
const isVerdict = (value: unknown): value is ReadVerdict => VERDICTS.includes(value as ReadVerdict)
const isNote = (value: unknown): value is string | null => value === null || isText(value)

function isRowState(value: unknown): boolean {
  if (value === 'written' || value === 'no_clause') return true
  if (!isRecord(value)) return false
  const [key, ...rest] = Object.keys(value)
  if (rest.length > 0 || key === undefined) return false
  const inner = value[key]
  if (!isRecord(inner)) return false
  switch (key) {
    case 'unwritten':
      return isText(inner.construct)
    case 'differs':
      return isText(inner.written)
    case 'by_hand':
      return isNote(inner.reason)
    default:
      return false
  }
}

function isRow(value: unknown): value is ReadRow {
  if (!isRecord(value) || !isCount(value.face) || !isRecord(value.span)) return false
  if (!isCount(value.span.start) || !isCount(value.span.end)) return false
  if (!isText(value.kind) || !isText(value.printed) || !isRowState(value.state)) return false
  if (!isListOf(value.abilities, isCount) || !isFlag(value.continues) || !isText(value.label)) return false
  if (!isNote(value.wording)) return false
  return isListOf(value.steps, (step): step is ReadRow['steps'][number] => {
    return isRecord(step) && isCount(step.depth) && isText(step.text)
  })
}

function readCard(body: unknown): ReadCard | null {
  if (!isRecord(body) || !isText(body.oracle_id) || !isText(body.name) || !isText(body.read_hash)) return null
  if (!isState(body.state) || !isCount(body.correct) || !isCount(body.wrong) || !isFlag(body.forge))
    return null
  const sheet = body.sheet
  if (!isRecord(sheet) || !isText(sheet.card) || !isListOf(sheet.faces, isText)) return null
  if (!isListOf(sheet.rows, isRow) || !isListOf(sheet.extra, isRow)) return null
  const mine = body.mine
  if (mine !== null) {
    if (!isRecord(mine) || !isVerdict(mine.verdict) || !isNote(mine.note)) return null
    if (!isText(mine.created_at) || !isFlag(mine.current)) return null
  }
  return body as ReadCard
}

function readQueue(body: unknown): ReadQueue | null {
  if (!isRecord(body) || !isText(body.title)) return null
  const fits = isListOf(body.cards, (card): card is ReadQueue['cards'][number] => {
    return (
      isRecord(card) &&
      isText(card.oracle_id) &&
      isText(card.name) &&
      isText(card.read_hash) &&
      isState(card.state) &&
      isCount(card.differs)
    )
  })
  return fits ? (body as ReadQueue) : null
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

/** The query string of a deck's list. */
function queueQuery(target: ReadTarget): string {
  return 'deck' in target
    ? `deck=${encodeURIComponent(target.deck)}`
    : `precon=${encodeURIComponent(target.precon)}`
}

/** Builds the client; call it once, or use the shared `readsApi`. */
export function createReadsApi(options: ReadsOptions = {}): ReadsApi {
  const base = options.baseUrl ?? ''
  const cardPath = (oracleId: string): string => `${base}/api/reads/${encodeURIComponent(oracleId)}`

  async function call<T>(path: string, read: (body: unknown) => T | null, init: RequestInit): Promise<T> {
    const send: Fetch = options.fetch ?? ((url, request) => globalThis.fetch(url, request))
    let response: Response
    let text: string
    try {
      response = await send(path, { credentials: 'include', ...init })
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
    queue: (target) => call(`${base}/api/reads?${queueQuery(target)}`, readQueue, {}),
    get: (oracleId) => call(cardPath(oracleId), readCard, {}),
    judge: (oracleId, body) =>
      call(cardPath(oracleId), readCard, {
        method: 'PUT',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(body),
      }),
  }
}

/** The client for the page origin, using the global `fetch`. */
export const readsApi: ReadsApi = createReadsApi()
