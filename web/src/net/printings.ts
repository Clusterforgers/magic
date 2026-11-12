/**
 * The art endpoints of `mtg-server` (docs/server.md, Art): a card's printings, the printing a stored deck shows for a
 * card's lines, and the art the account prefers for a card where a deck line chooses none. Every answer is checked at
 * runtime and every failure is a `DeckError` with a stable code. Every request carries the session cookie.
 */
import type { DeckZone, LinePrintingBody, PreferredPrintingBody, PrintingView } from '@/protocol/generated'
import type { Fetch } from './api'
import { isRecord, isText } from './check'
import { DeckError, isDeckErrorCode } from './decks'

/** Options for `createPrintingsApi`. */
export interface PrintingsOptions {
  /** Defaults to the global `fetch`, looked up at call time. */
  fetch?: Fetch
  /** Prefix for every path; empty means the page origin. */
  baseUrl?: string
}

/** The art calls; each rejects with a `DeckError`. */
export interface PrintingsApi {
  /** `GET /api/cards/{oracle_id}/printings`: the card's printings, newest first. */
  printingsOf(oracleId: string, signal?: AbortSignal): Promise<PrintingView[]>
  /** `PUT /api/decks/{id}/printing`: the art of the card's lines in `zone`; null for the default. */
  setLine(deckId: string, zone: DeckZone, oracleId: string, printingId: string | null): Promise<void>
  /** `GET /api/me/printings`: printing id by oracle id. */
  preferred(): Promise<Record<string, string>>
  /** `PUT /api/me/printings/{oracle_id}`, or `DELETE` for null: the art the account plays the card in by default. */
  prefer(oracleId: string, printingId: string | null): Promise<void>
}

const isNullableText = (value: unknown): value is string | null => value === null || isText(value)

/** Reads one printing; the look fields an older server leaves out read as unknown. */
export function readPrinting(item: unknown): PrintingView | null {
  if (!isRecord(item) || !isText(item.printing_id) || !isText(item.set) || !isText(item.collector_number))
    return null
  if (!isText(item.lang) || typeof item.digital !== 'boolean' || !isRecord(item.images)) return null
  const rarity = item.rarity ?? null
  const released = item.released_at ?? null
  const setName = item.set_name ?? null
  const frame = item.frame ?? null
  if (!isNullableText(rarity) || !isNullableText(released) || !isNullableText(setName) || !isNullableText(frame))
    return null
  const finishes = Array.isArray(item.finishes) ? item.finishes.filter(isText) : []
  return {
    printing_id: item.printing_id,
    set: item.set,
    collector_number: item.collector_number,
    rarity,
    released_at: released,
    lang: item.lang,
    digital: item.digital,
    images: item.images as PrintingView['images'],
    set_name: setName,
    frame,
    finishes,
    full_art: item.full_art === true,
  }
}

function readList(body: unknown): PrintingView[] | null {
  if (!isRecord(body) || !Array.isArray(body.printings)) return null
  const out: PrintingView[] = []
  for (const item of body.printings) {
    const printing = readPrinting(item)
    if (printing === null) return null
    out.push(printing)
  }
  return out
}

function readPreferred(body: unknown): Record<string, string> | null {
  if (!isRecord(body) || !isRecord(body.printings)) return null
  const out: Record<string, string> = {}
  for (const [card, printing] of Object.entries(body.printings)) {
    if (!isText(printing)) return null
    out[card] = printing
  }
  return out
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

const json = (method: string, body: unknown): RequestInit => ({
  method,
  headers: { 'content-type': 'application/json' },
  body: JSON.stringify(body),
})

/** Builds the client; call it once, or use the shared `printingsApi`. */
export function createPrintingsApi(options: PrintingsOptions = {}): PrintingsApi {
  const base = options.baseUrl ?? ''
  const enc = encodeURIComponent

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

  const done = (): true => true
  return {
    printingsOf: (oracleId, signal) =>
      call(`${base}/api/cards/${enc(oracleId)}/printings`, readList, signal === undefined ? {} : { signal }),
    setLine: async (deckId, zone, oracleId, printingId) => {
      const body: LinePrintingBody = { zone, oracle_id: oracleId, printing_id: printingId }
      await call(`${base}/api/decks/${enc(deckId)}/printing`, done, json('PUT', body))
    },
    preferred: () => call(`${base}/api/me/printings`, readPreferred, {}),
    prefer: async (oracleId, printingId) => {
      const path = `${base}/api/me/printings/${enc(oracleId)}`
      if (printingId === null) {
        await call(path, done, { method: 'DELETE' })
        return
      }
      const body: PreferredPrintingBody = { printing_id: printingId }
      await call(path, done, json('PUT', body))
    },
  }
}

/** The client for the page origin, using the global `fetch`. */
export const printingsApi: PrintingsApi = createPrintingsApi()
