/**
 * The card support endpoints of `mtg-server` (docs/server.md, Card support): the newest coverage snapshot with its trend,
 * the newest daily check and the feed (`GET /api/card-support`), and the cards that do not play automatically
 * (`GET /api/card-support/missing`). Every answer is checked at runtime; a failure is a `DeckError`, since the endpoints
 * share the deck endpoints' codes. Every request carries the session cookie (`credentials: 'include'`).
 */
import type {
  CardSupport,
  FeedEvent,
  MissingCardView,
  MissingCards,
  RefreshCheck,
  SupportBucket,
  SupportPoint,
  SupportSnapshot,
} from '@/protocol/generated'
import type { Fetch } from './api'
import { isCount, isFlag, isListOf, isRecord, isText } from './check'
import { DeckError, isDeckErrorCode } from './decks'

/** Options for `createCardSupportApi`. */
export interface CardSupportOptions {
  /** Defaults to the global `fetch`, looked up at call time. */
  fetch?: Fetch
  /** Prefix for every path; empty means the page origin. */
  baseUrl?: string
}

/** The card support calls; each rejects with a `DeckError`. */
export interface CardSupportApi {
  /** `GET /api/card-support`. */
  overview(): Promise<CardSupport>
  /** `GET /api/card-support/missing`. */
  missing(): Promise<MissingCards>
}

const isOptText = (value: unknown): value is string | null => value === null || isText(value)
const isOptCount = (value: unknown): value is number | null => value === null || isCount(value)
const isTexts = (value: unknown): value is string[] => isListOf(value, isText)

const STATES = ['verified', 'read', 'unverified', 'waiting', 'partial', 'manual', 'other'] as const

function isSnapshot(value: unknown): value is SupportSnapshot {
  if (!isRecord(value) || !isRecord(value.states)) return false
  const states = value.states
  return (
    isCount(value.id) &&
    isText(value.taken_at) &&
    isOptText(value.release) &&
    isCount(value.cards) &&
    isCount(value.automatic) &&
    isCount(value.missing) &&
    STATES.every((state) => isCount(states[state])) &&
    isListOf(value.buckets, isBucket)
  )
}

const isBucket = (value: unknown): value is SupportBucket =>
  isRecord(value) &&
  isText(value.label) &&
  isCount(value.cards) &&
  isCount(value.verified) &&
  isCount(value.automatic)

const isPoint = (value: unknown): value is SupportPoint =>
  isRecord(value) &&
  isText(value.taken_at) &&
  isOptText(value.release) &&
  isCount(value.cards) &&
  isCount(value.automatic)

const isCheck = (value: unknown): value is RefreshCheck =>
  isRecord(value) &&
  isText(value.started_at) &&
  isOptText(value.finished_at) &&
  (value.state === 'running' ||
    value.state === 'succeeded' ||
    value.state === 'deferred' ||
    value.state === 'failed') &&
  isOptText(value.scryfall_updated_at) &&
  isOptText(value.mtgjson_build)

function isEvent(value: unknown): value is FeedEvent {
  if (!isRecord(value) || !isText(value.at)) return false
  switch (value.kind) {
    case 'new_cards':
      return isCount(value.count) && isTexts(value.names)
    case 'new_precons':
      return isText(value.code) && isOptText(value.set_name) && isTexts(value.decks)
    case 'now_automatic':
      return (
        isOptText(value.release) && isFlag(value.new_release) && isCount(value.count) && isTexts(value.names)
      )
    case 'check_failed':
      return isTexts(value.failures)
    default:
      return false
  }
}

const isMissing = (value: unknown): value is MissingCardView =>
  isRecord(value) &&
  isText(value.oracle_id) &&
  isText(value.name) &&
  (value.state === 'waiting' ||
    value.state === 'partial' ||
    value.state === 'manual' ||
    value.state === 'other') &&
  isText(value.blocker) &&
  isText(value.detail) &&
  isCount(value.blockers) &&
  isOptCount(value.rank)

/** The overview, or null when the body is not one. */
export function readOverview(body: unknown): CardSupport | null {
  if (!isRecord(body)) return null
  const { snapshot, history, last_check: check, feed } = body
  if (snapshot !== null && !isSnapshot(snapshot)) return null
  if (check !== null && !isCheck(check)) return null
  if (!isListOf(history, isPoint) || !isListOf(feed, isEvent)) return null
  return { snapshot, history, last_check: check, feed }
}

/** The missing cards, or null when the body is not a list of them. */
export function readMissing(body: unknown): MissingCards | null {
  if (!isRecord(body) || !isOptCount(body.snapshot_id) || !isListOf(body.cards, isMissing)) return null
  return { snapshot_id: body.snapshot_id, cards: body.cards }
}

function parseJson(text: string): unknown {
  try {
    return JSON.parse(text)
  } catch {
    return undefined
  }
}

/** Builds the client; call it once, or use the shared `cardSupportApi`. */
export function createCardSupportApi(options: CardSupportOptions = {}): CardSupportApi {
  const base = options.baseUrl ?? ''

  async function call<T>(path: string, read: (body: unknown) => T | null): Promise<T> {
    const send: Fetch = options.fetch ?? ((url, request) => globalThis.fetch(url, request))
    let response: Response
    let text: string
    try {
      response = await send(`${base}${path}`, { credentials: 'include' })
      text = await response.text()
    } catch {
      throw new DeckError('network', 0, 'Could not reach the server. Check your connection and try again.')
    }
    const body = parseJson(text)
    if (!response.ok) {
      if (isRecord(body) && isText(body.error) && isText(body.message)) {
        throw new DeckError(
          isDeckErrorCode(body.error) ? body.error : 'unknown',
          response.status,
          body.message,
        )
      }
      throw new DeckError(
        'bad_response',
        response.status,
        `The server answered with HTTP ${response.status}.`,
      )
    }
    const value = read(body)
    if (value === null) {
      throw new DeckError('bad_response', response.status, 'The server sent an answer this page cannot read.')
    }
    return value
  }

  return {
    overview: () => call('/api/card-support', readOverview),
    missing: () => call('/api/card-support/missing', readMissing),
  }
}

/** The client for the page origin, using the global `fetch`. */
export const cardSupportApi: CardSupportApi = createCardSupportApi()
