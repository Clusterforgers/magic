/**
 * The bulk read endpoints of `mtg-server` (docs/server.md, Bulk reads): the drafts whose clauses differ from the card
 * only in wording, and one approval of the ones the player checked. Answers are checked at runtime; every failure is a
 * `DeckError`. Every request carries the session cookie.
 */
import type { BulkApprovalBody, BulkApprovalResult, BulkReads } from '@/protocol/generated'
import type { Fetch } from './api'
import { isListOf, isRecord, isText } from './check'
import { DeckError, isDeckErrorCode } from './decks'

/** Options for `createBulkReadsApi`. */
export interface BulkReadsOptions {
  /** Defaults to the global `fetch`, looked up at call time. */
  fetch?: Fetch
  /** Prefix for every path; empty means the page origin. */
  baseUrl?: string
}

/** The bulk read calls; each rejects with a `DeckError`. */
export interface BulkReadsApi {
  /** `GET /api/reads/bulk`: the cards the account may approve in bulk, by name, with the rules they need. */
  list(): Promise<BulkReads>
  /** `PUT /api/reads/bulk`: one `correct` verdict per named card still on the named hash. */
  approve(body: BulkApprovalBody): Promise<BulkApprovalResult>
}

const isTexts = (value: unknown): value is string[] => isListOf(value, isText)

function bulkReads(body: unknown): BulkReads | null {
  if (!isRecord(body)) return null
  const rules = isListOf(body.rules, (rule): rule is BulkReads['rules'][number] => {
    return isRecord(rule) && isText(rule.name) && isText(rule.summary)
  })
  const cards = isListOf(body.cards, (card): card is BulkReads['cards'][number] => {
    if (!isRecord(card) || !isText(card.oracle_id) || !isText(card.name)) return false
    if (!isText(card.read_hash) || !isText(card.rule)) return false
    return isListOf(card.clauses, (clause): clause is BulkReads['cards'][number]['clauses'][number] => {
      return isRecord(clause) && isText(clause.printed) && isText(clause.engine) && isTexts(clause.rules)
    })
  })
  return rules && cards ? (body as BulkReads) : null
}

function approvalResult(body: unknown): BulkApprovalResult | null {
  if (!isRecord(body) || !isTexts(body.approved) || !isTexts(body.stale)) return null
  return body as BulkApprovalResult
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

/** Builds the client; call it once, or use the shared `bulkReadsApi`. */
export function createBulkReadsApi(options: BulkReadsOptions = {}): BulkReadsApi {
  const path = `${options.baseUrl ?? ''}/api/reads/bulk`

  async function call<T>(read: (body: unknown) => T | null, init: RequestInit): Promise<T> {
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
    list: () => call(bulkReads, {}),
    approve: (body) =>
      call(approvalResult, {
        method: 'PUT',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(body),
      }),
  }
}

/** The client for the page origin, using the global `fetch`. */
export const bulkReadsApi: BulkReadsApi = createBulkReadsApi()
