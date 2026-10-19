/** Typed fixtures of the card wire types and a card server in memory, for tests of the client and the builder. */
import { cardHit, deckError } from '@/features/decks/fakeDecks'
import type { Fetch } from '@/net/api'
import { createCardsApi } from '@/net/cards'
import type { CardsApi } from '@/net/cards'
import type { CardDetail, CardHit, CardSearchPage } from '@/protocol/generated'
import { fakeFetch, jsonResponse } from '@/test-utils/fakeFetch'
import type { FetchCall } from '@/test-utils/fakeFetch'

export { cardHit }

/** One page of a search. */
export const searchPage = (cards: CardHit[], over: Partial<CardSearchPage> = {}): CardSearchPage => ({
  cards,
  total: cards.length,
  page: 1,
  per_page: 40,
  ...over,
})

/** The answer of `GET /api/cards/{oracle_id}` for a one-face instant. */
export const cardDetail = (name = 'Lightning Bolt', over: Partial<CardDetail> = {}): CardDetail => ({
  card: cardHit(name),
  oracle_text: `${name} deals 3 damage to any target.`,
  keywords: [],
  faces: [
    {
      name,
      mana_cost: '{R}',
      type_line: 'Instant',
      oracle_text: `${name} deals 3 damage to any target.`,
      power: null,
      toughness: null,
      loyalty: null,
      defense: null,
      colors: ['R'],
    },
  ],
  legalities: { modern: 'legal', standard: 'not_legal' },
  printings: [],
  rulings: [],
  reasons: [],
  ...over,
})

/** The first `count` cards of a made-up pool, named `Card 001`, `Card 002` and so on. */
export const pool = (count: number, over: (index: number) => Partial<CardHit> = () => ({})): CardHit[] =>
  Array.from({ length: count }, (_, index) =>
    cardHit(`Card ${String(index + 1).padStart(3, '0')}`, { mana_value: index % 8, ...over(index) }),
  )

type Answer = (call: FetchCall) => Response | Promise<Response> | undefined

/** What the fake card server does beyond the usual; leave a field out for the usual good behaviour. */
export interface CardsScript {
  /** The cards the database holds, in the order a search lists them. */
  cards?: CardHit[]
  /** The details by oracle id for `GET /api/cards/{oracle_id}`. */
  details?: CardDetail[]
  /** The server has no card database: every call is 503 `cards_unavailable`. */
  unavailable?: boolean
  /** Replaces the answer of one endpoint. */
  override?: Partial<Record<'search' | 'get', Answer>>
}

/** A card server in memory, with a record of what it was asked. */
export interface CardsServer {
  api: CardsApi
  fetch: Fetch
  calls: FetchCall[]
  /** The query string of every search, parsed, oldest first. */
  searches: URLSearchParams[]
}

// The server's `cmc` forms: `3`, `2-4`, `5+`.
function valueMatches(value: number, spec: string): boolean {
  const range = /^(\d+)-(\d+)$/.exec(spec)
  if (range !== null) return value >= Number(range[1]) && value <= Number(range[2])
  if (spec.endsWith('+')) return value >= Number(spec.slice(0, -1))
  return value === Number(spec)
}

function matches(card: CardHit, params: URLSearchParams): boolean {
  const q = params.get('q')?.trim().toLowerCase() ?? ''
  if (q !== '' && !card.name.toLowerCase().includes(q)) return false
  const identity = params.get('identity')?.toUpperCase() ?? ''
  if (identity !== '' && !card.color_identity.every((color) => identity.includes(color))) return false
  const words = (params.get('type') ?? '')
    .toLowerCase()
    .split(/\s+/)
    .filter((word) => word !== '')
  if (!words.every((word) => card.type_line.toLowerCase().includes(word))) return false
  const cmc = params.get('cmc') ?? ''
  return cmc === '' || valueMatches(card.mana_value, cmc)
}

/** A card server in memory: a search filters the pool the way `GET /api/cards` does for the fields the builder sends. */
export function startCards(script: CardsScript = {}): CardsServer {
  const cards = script.cards ?? []
  const searches: URLSearchParams[] = []
  const fake = fakeFetch((call) => {
    if (script.unavailable === true)
      return deckError(503, 'cards_unavailable', 'this server has no card database')
    const url = new URL(call.url, 'http://server.test')
    if (url.pathname === '/api/cards') {
      searches.push(url.searchParams)
      const custom = script.override?.search?.(call)
      if (custom !== undefined) return custom
      const found = cards.filter((card) => matches(card, url.searchParams))
      const perPage = Math.min(Number(url.searchParams.get('per_page') ?? 40), 100)
      const page = Number(url.searchParams.get('page') ?? 1)
      const from = (page - 1) * perPage
      return jsonResponse(
        200,
        searchPage(found.slice(from, from + perPage), { total: found.length, page, per_page: perPage }),
      )
    }
    if (url.pathname.startsWith('/api/cards/')) {
      const custom = script.override?.get?.(call)
      if (custom !== undefined) return custom
      const id = decodeURIComponent(url.pathname.slice('/api/cards/'.length))
      const detail = (script.details ?? []).find((item) => item.card.oracle_id === id)
      return detail === undefined ? deckError(404, 'not_found', 'no such card') : jsonResponse(200, detail)
    }
    return deckError(404, 'not_found', 'no such resource')
  })
  return { api: createCardsApi({ fetch: fake.fetch }), fetch: fake.fetch, calls: fake.calls, searches }
}
