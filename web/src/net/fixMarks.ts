/**
 * The fix-mark endpoints of `mtg-server` (docs/server.md, Fix marks): list the account's marks, mark a card as important
 * to fix with an optional note, and take the mark back. The bodies are the generated types of `mtg_protocol::rest`;
 * every answer is checked at runtime and every failure is a `DeckError` with a stable code, since the endpoints share the
 * deck endpoints' codes. Every request carries the session cookie (`credentials: 'include'`).
 */
import type { FixMarkBody, FixMarkView } from '@/protocol/generated'
import type { Fetch } from './api'
import { isRecord, isText } from './check'
import { DeckError, isDeckErrorCode } from './decks'

/** Options for `createFixMarksApi`. */
export interface FixMarksOptions {
  /** Defaults to the global `fetch`, looked up at call time. */
  fetch?: Fetch
  /** Prefix for every path; empty means the page origin, which Vite proxies to the server in development. */
  baseUrl?: string
}

/** The fix-mark calls; each rejects with a `DeckError`. */
export interface FixMarksApi {
  /** `GET /api/fix-marks`: the account's marks, the newest first. */
  list(): Promise<FixMarkView[]>
  /** `PUT /api/fix-marks/{oracle_id}`: marks the card, or replaces the note of the mark that exists. */
  mark(oracleId: string, note?: string | null): Promise<FixMarkView>
  /** `DELETE /api/fix-marks/{oracle_id}`: takes the account's mark back; resolves when it was already gone. */
  unmark(oracleId: string): Promise<void>
}

function readView(item: unknown): FixMarkView | null {
  if (!isRecord(item) || !isText(item.oracle_id) || !isText(item.name) || !isText(item.created_at))
    return null
  if (item.note !== null && !isText(item.note)) return null
  return { oracle_id: item.oracle_id, name: item.name, note: item.note, created_at: item.created_at }
}

function readList(body: unknown): FixMarkView[] | null {
  if (!isRecord(body) || !Array.isArray(body.marks)) return null
  const marks: FixMarkView[] = []
  for (const item of body.marks) {
    const mark = readView(item)
    if (mark === null) return null
    marks.push(mark)
  }
  return marks
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

/** Builds the client; call it once, or use the shared `fixMarksApi`. */
export function createFixMarksApi(options: FixMarksOptions = {}): FixMarksApi {
  const base = options.baseUrl ?? ''
  const markPath = (oracleId: string): string => `${base}/api/fix-marks/${encodeURIComponent(oracleId)}`

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
    list: () => call(`${base}/api/fix-marks`, readList, {}),
    mark: (oracleId, note = null) => {
      const body: FixMarkBody = { note }
      return call(markPath(oracleId), readView, {
        method: 'PUT',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(body),
      })
    },
    unmark: async (oracleId) => {
      try {
        await call(markPath(oracleId), () => true, { method: 'DELETE' })
      } catch (error) {
        // A mark that is already gone is what the caller wanted.
        if (!(error instanceof DeckError && error.status === 404)) throw error
      }
    },
  }
}

/** The client for the page origin, using the global `fetch`. */
export const fixMarksApi: FixMarksApi = createFixMarksApi()
