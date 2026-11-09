/**
 * A deck-check endpoint in memory, laid over a deck server's `fetch` for tests and for `/dev/builder`: `POST
 * /api/decks/check` answers the report a server would work out for the list it is sent, and every other request goes to
 * the server underneath.
 */
import { reportCard, reportOf } from '@/features/decks/fakeDecks'
import type { Fetch } from '@/net/api'
import type { CardHit, CheckRequest, DeckReport, DeckZone, Issue, Reason } from '@/protocol/generated'
import { jsonResponse } from '@/test-utils/fakeFetch'

/** The reason the server gives a card its deck's owner plays by hand (`OWNER_BY_HAND` in `mtg-cards`). */
export const OWNER_BY_HAND = "the deck's owner chose to play it by hand"

/** What the fake check knows and does beyond the usual; leave a field out for the usual good behaviour. */
export interface CheckScript {
  /** The cards the check knows by oracle id, with how far the game plays each; a card left out counts as verified. */
  cards?: readonly CardHit[]
  /** The clauses left to the table, by oracle id. */
  reasons?: Readonly<Record<string, readonly Reason[]>>
  /** The ways a list breaks the rules of its format; none by default. */
  legality?: (request: CheckRequest) => Issue[]
  /** Milliseconds every answer waits, so a page can be seen while it checks. */
  delayMs?: number
  /** Replaces the answer to a request (a failure, or one that waits on the test); undefined gives the usual. */
  answer?: (
    request: CheckRequest,
    signal: AbortSignal | undefined,
  ) => Response | Promise<Response> | undefined
}

/** A check endpoint in memory, with a record of what it was asked. */
export interface CheckServer {
  /** The `fetch` to build the deck client on: the check here, everything else underneath. */
  fetch: Fetch
  /** Every list the check was asked about, oldest first. */
  requests: CheckRequest[]
  /** The signal each request carried, in the same order; it aborts when the page no longer wants the answer. */
  signals: Array<AbortSignal | undefined>
}

const ZONE_ORDER: readonly DeckZone[] = ['main', 'sideboard', 'commander', 'companion']

/** The report of the cards of `request`, by zone and name as the server orders it. */
export function reportFor(request: CheckRequest, script: CheckScript = {}): DeckReport {
  const known = new Map((script.cards ?? []).map((card) => [card.oracle_id, card]))
  // As on the server, a card is played by hand when any of its lines says so.
  const byHand = new Set((request.cards ?? []).filter((line) => line.by_hand === true).map((line) => line.oracle_id))
  const cards = (request.cards ?? [])
    .map((line) => {
      const card = known.get(line.oracle_id)
      const support = card?.support ?? 'verified'
      const chosen = byHand.has(line.oracle_id) && support !== 'unplayable'
      return reportCard(card?.name ?? line.oracle_id, {
        oracle_id: line.oracle_id,
        zone: line.zone,
        count: line.count,
        support: chosen ? 'manual' : support,
        reasons: chosen
          ? [{ face: 0, span: { start: 0, end: 0 }, text: OWNER_BY_HAND }]
          : [...(script.reasons?.[line.oracle_id] ?? [])],
        ...(chosen ? { by_hand: true } : {}),
      })
    })
    .sort(
      (a, b) =>
        ZONE_ORDER.indexOf(a.zone) - ZONE_ORDER.indexOf(b.zone) ||
        a.name.localeCompare(b.name, 'en', { sensitivity: 'base' }),
    )
  return reportOf(cards, script.legality?.(request) ?? [], { format: request.format ?? 'commander' })
}

// Waits like a request in flight: a signal that aborts ends it the way `fetch` would.
function wait(ms: number, signal: AbortSignal | undefined): Promise<void> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(resolve, ms)
    signal?.addEventListener('abort', () => {
      clearTimeout(timer)
      reject(new DOMException('The operation was aborted.', 'AbortError'))
    })
  })
}

/** Lays the check endpoint over `inner`, which serves everything else. */
export function withCheck(inner: Fetch, script: CheckScript = {}): CheckServer {
  const requests: CheckRequest[] = []
  const signals: Array<AbortSignal | undefined> = []
  const fetch: Fetch = async (url, init) => {
    if (!url.split('?')[0]?.endsWith('/api/decks/check') || init?.method !== 'POST') return inner(url, init)
    // The page sends well-formed bodies; the cast names what the parsed JSON is taken to be.
    const request: CheckRequest = JSON.parse(typeof init.body === 'string' ? init.body : 'null')
    const signal = init.signal ?? undefined
    requests.push(request)
    signals.push(signal)
    const custom = script.answer?.(request, signal)
    if (custom !== undefined) return custom
    if (script.delayMs !== undefined) await wait(script.delayMs, signal)
    return jsonResponse(200, reportFor(request, script))
  }
  return { fetch, requests, signals }
}
