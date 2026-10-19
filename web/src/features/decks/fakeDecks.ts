/** Typed fixtures of the deck wire types and a deck server in memory, for tests of the client and the pages. */
import type { Fetch } from '@/net/api'
import { createDecksApi } from '@/net/decks'
import type { DecksApi } from '@/net/decks'
import type {
  CardHit,
  Counts,
  DeckBody,
  DeckErrorCode,
  DeckLineView,
  DeckListItem,
  DeckState,
  DeckView,
  ImportRequest,
  ImportResult,
  Summary,
} from '@/protocol/generated'
import { fakeFetch, jsonResponse } from '@/test-utils/fakeFetch'
import type { FetchCall } from '@/test-utils/fakeFetch'

/** Copies by support state; a clean hundred-card deck unless told otherwise. */
export const counts = (over: Partial<Counts> = {}): Counts => ({
  total: 100,
  verified: 100,
  partial: 0,
  manual: 0,
  stale: 0,
  unplayable: 0,
  ...over,
})

/** The summary of a check. */
export const summary = (over: Partial<Summary> = {}): Summary => ({
  format: 'commander',
  state: 'verified',
  counts: counts(),
  zones: [{ zone: 'main', counts: counts() }],
  issues: 0,
  ...over,
})

/** A deck of the list: verified, a hundred cards, saved on 17 October 2026. */
export const deckItem = (over: Partial<DeckListItem> = {}): DeckListItem => ({
  id: 'deck-1',
  name: 'Atraxa Superfriends',
  format: 'commander',
  created_at: '2026-10-10T09:00:00Z',
  updated_at: '2026-10-17T12:00:00Z',
  state: 'verified',
  verified: true,
  changed: false,
  checked_at: '2026-10-17T12:00:00Z',
  summary: summary(),
  ...over,
})

/** A deck of the list in `state`, with the fields that go with it. */
export function deckIn(state: DeckState, over: Partial<DeckListItem> = {}): DeckListItem {
  const verified = state === 'verified'
  return deckItem({
    state,
    verified,
    changed: state === 'unchecked',
    summary: summary({ state: state === 'unchecked' ? 'verified' : state }),
    ...over,
  })
}

/** A card as a search shows it. */
export const cardHit = (name: string, over: Partial<CardHit> = {}): CardHit => ({
  oracle_id: `oracle-${name.toLowerCase().replace(/\W+/g, '-')}`,
  name,
  mana_cost: '{R}',
  mana_value: 1,
  type_line: 'Instant',
  colors: ['R'],
  color_identity: ['R'],
  images: { card: null, faces: [] },
  support: 'verified',
  ...over,
})

/** A line of a deck or an import: `count` copies of the card called `name`. */
export const lineOf = (name: string, count = 1, over: Partial<DeckLineView> = {}): DeckLineView => ({
  zone: 'main',
  count,
  printing_id: null,
  card: cardHit(name),
  ...over,
})

/** A deck as `GET /api/decks/{id}` shows it. */
export const deckView = (over: Partial<DeckView> = {}): DeckView => ({
  id: 'deck-1',
  name: 'Atraxa Superfriends',
  format: 'commander',
  notes: '',
  created_at: '2026-10-10T09:00:00Z',
  updated_at: '2026-10-17T12:00:00Z',
  cards: [lineOf('Lightning Bolt', 4)],
  state: 'verified',
  verified: true,
  changed: false,
  check: null,
  report: {
    deck_hash: 'a'.repeat(64),
    coverage_hash: 'b'.repeat(64),
    format: 'commander',
    legality: [],
    cards: [],
    summary: summary(),
  },
  ...over,
})

/** What `POST /api/decks/import` answers for a clean Arena list. */
export const importResult = (over: Partial<ImportResult> = {}): ImportResult => ({
  source: 'arena',
  cards: [lineOf('Lightning Bolt', 4), lineOf('Mountain', 20)],
  unresolved: [],
  problems: [],
  ignored: [],
  ...over,
})

/** The error body of `mtg-server`. */
export const deckError = (status: number, error: DeckErrorCode, message = 'refused'): Response =>
  jsonResponse(status, { error, message })

type Answer = (call: FetchCall) => Response | Promise<Response>

/** What the fake deck server does beyond the usual; leave a field out for the usual good behaviour. */
export interface DecksScript {
  /** The decks the account holds when the page opens, most recently saved first. */
  decks?: DeckListItem[]
  /** The server has no database: every call is 503 `decks_unavailable`. */
  unavailable?: boolean
  /** What `POST /api/decks/import` reads the pasted list as. */
  importResult?: ImportResult
  /** Replaces the answer of one endpoint. */
  override?: Partial<Record<'list' | 'create' | 'import' | 'remove', Answer>>
}

/** A deck server in memory, with a record of what it was asked. */
export interface DecksServer {
  api: DecksApi
  fetch: Fetch
  calls: FetchCall[]
  /** The decks held now, most recently saved first. */
  readonly decks: readonly DeckListItem[]
  /** The parsed body of every save. */
  created: DeckBody[]
  /** The parsed body of every import. */
  imports: ImportRequest[]
  /** The ids of the decks deleted. */
  removed: string[]
}

/** A deck server in memory: saving puts a deck first, deleting removes it, importing answers `importResult`. */
export function startDecks(script: DecksScript = {}): DecksServer {
  let held = [...(script.decks ?? [])]
  const created: DeckBody[] = []
  const imports: ImportRequest[] = []
  const removed: string[] = []
  let made = 0
  // The tests send well-formed bodies; the generic names what the parsed JSON is taken to be.
  const bodyOf = <T>(call: FetchCall): T =>
    JSON.parse(typeof call.init?.body === 'string' ? call.init.body : 'null')

  const fake = fakeFetch((call) => {
    if (script.unavailable === true) return deckError(503, 'decks_unavailable', 'this server has no database')
    const path = call.url.split('?')[0] ?? call.url
    const method = call.init?.method ?? 'GET'
    if (path === '/api/decks' && method === 'GET') {
      return script.override?.list?.(call) ?? jsonResponse(200, { decks: held })
    }
    if (path === '/api/decks' && method === 'POST') {
      const body = bodyOf<DeckBody>(call)
      created.push(body)
      if (script.override?.create !== undefined) return script.override.create(call)
      made += 1
      const copies = body.cards.reduce((sum, line) => sum + line.count, 0)
      const state: DeckState = copies === 0 ? 'blocked' : 'verified'
      const saved = deckItem({
        id: `made-${made}`,
        name: body.name,
        format: body.format,
        state,
        verified: state === 'verified',
        summary: summary({
          format: body.format,
          state,
          counts: counts({ total: copies, verified: copies }),
          issues: copies === 0 ? 1 : 0,
        }),
      })
      held = [saved, ...held]
      return jsonResponse(
        201,
        deckView({ id: saved.id, name: saved.name, format: saved.format, state, verified: saved.verified }),
      )
    }
    if (path === '/api/decks/import' && method === 'POST') {
      imports.push(bodyOf<ImportRequest>(call))
      return script.override?.import?.(call) ?? jsonResponse(200, script.importResult ?? importResult())
    }
    if (path.startsWith('/api/decks/') && method === 'DELETE') {
      if (script.override?.remove !== undefined) return script.override.remove(call)
      const id = decodeURIComponent(path.slice('/api/decks/'.length))
      if (!held.some((deck) => deck.id === id)) return deckError(404, 'not_found', 'no such deck')
      held = held.filter((deck) => deck.id !== id)
      removed.push(id)
      return new Response(null, { status: 204 })
    }
    return deckError(404, 'not_found', 'no such resource')
  })

  return {
    api: createDecksApi({ fetch: fake.fetch }),
    fetch: fake.fetch,
    calls: fake.calls,
    get decks() {
      return held
    },
    created,
    imports,
    removed,
  }
}
