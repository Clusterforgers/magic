// Goal: the card client builds the search address from the filters (blank fields left out, values trimmed and escaped, paging when given), returns only answers that match the generated types, passes the abort signal on, and turns every failure into a CardsError whose code is the server's DeckErrorCode or a local unknown, network or bad_response, 503 cards_unavailable included.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cardDetail, cardHit, searchPage } from '@/features/builder/fakeCards'
import { deckError } from '@/features/decks/fakeDecks'
import type { DeckErrorCode } from '@/protocol/generated'
import { fakeFetch, jsonResponse } from '@/test-utils/fakeFetch'
import type { FetchCall } from '@/test-utils/fakeFetch'
import { cardsApi, CardsError, createCardsApi, isCardsUnavailable, searchParams } from './cards'
import type { CardQuery, CardsApi } from './cards'
import { DECK_ERROR_CODES } from './decks'

afterEach(() => {
  vi.unstubAllGlobals()
})

/** The client over a fetch that answers every request with `answer`. */
function serve(answer: (call: FetchCall) => Response | Promise<Response>, baseUrl?: string) {
  const fake = fakeFetch(answer)
  return { api: createCardsApi({ fetch: fake.fetch, baseUrl }), calls: fake.calls }
}

const CALLS: ReadonlyArray<readonly [string, (api: CardsApi) => Promise<unknown>]> = [
  ['search', (api) => api.search({ text: 'bolt' })],
  ['get', (api) => api.get('oracle-lightning-bolt')],
]

describe('the search address', () => {
  it.each<[string, CardQuery, string]>([
    ['nothing', {}, ''],
    ['text', { text: 'lightning bolt' }, 'q=lightning+bolt'],
    ['a colour identity', { identity: 'BG' }, 'identity=BG'],
    ['a type', { type: 'legendary creature' }, 'type=legendary+creature'],
    ['a mana value, a range and a minimum', { manaValue: '2-4' }, 'cmc=2-4'],
    ['a format', { format: 'commander' }, 'format=commander'],
    ['a page and its size', { page: 3, perPage: 20 }, 'page=3&per_page=20'],
    [
      'every field in a fixed order',
      { page: 2, format: 'modern', manaValue: '5+', type: 'elf', identity: 'G', text: 'llanowar' },
      'q=llanowar&identity=G&type=elf&cmc=5%2B&format=modern&page=2',
    ],
  ])('for %s is %j', (_what, query, expected) => {
    expect(searchParams(query).toString()).toBe(expected)
  })

  it('leaves out blank and whitespace-only fields and trims the rest', () => {
    const query = { text: '  ', identity: '', type: ' \t ', manaValue: '', format: ' modern ' }
    expect(searchParams(query).toString()).toBe('format=modern')
  })

  it('escapes what would break the query string', () => {
    expect(searchParams({ text: 'a&b=c #1' }).toString()).toBe('q=a%26b%3Dc+%231')
  })
})

describe('the requests', () => {
  it('searches without the session cookie and with no body, and returns the page', async () => {
    const page = searchPage([cardHit('Lightning Bolt'), cardHit('Chain Lightning')], { total: 87, page: 2 })
    const { api, calls } = serve(() => jsonResponse(200, page))
    expect(await api.search({ text: 'lightning', page: 2 })).toEqual(page)
    expect(calls).toHaveLength(1)
    expect(calls[0]?.url).toBe('/api/cards?q=lightning&page=2')
    expect(calls[0]?.init).toBeUndefined()
  })

  it('asks the bare address when there is nothing to filter by', async () => {
    const { api, calls } = serve(() => jsonResponse(200, searchPage([])))
    await api.search({})
    expect(calls[0]?.url).toBe('/api/cards')
  })

  it('hands the abort signal to fetch', async () => {
    const { api, calls } = serve(() => jsonResponse(200, searchPage([])))
    const controller = new AbortController()
    await api.search({ text: 'x' }, controller.signal)
    await api.get('oracle-x', controller.signal).catch(() => undefined)
    expect(calls[0]?.init?.signal).toBe(controller.signal)
    expect(calls[1]?.init?.signal).toBe(controller.signal)
  })

  it('reads one card by an id that is escaped into the path', async () => {
    const detail = cardDetail('Lightning Bolt')
    const { api, calls } = serve(() => jsonResponse(200, detail))
    expect(await api.get('a b/c')).toEqual(detail)
    expect(calls[0]?.url).toBe('/api/cards/a%20b%2Fc')
  })

  it('puts the base address in front of every path', async () => {
    const { api, calls } = serve(() => jsonResponse(200, searchPage([])), 'http://game.test')
    await api.search({ text: 'x' })
    expect(calls[0]?.url).toBe('http://game.test/api/cards?q=x')
  })

  it('uses the global fetch of the moment for the shared client', async () => {
    const fake = fakeFetch(() => jsonResponse(200, searchPage([cardHit('Island')])))
    vi.stubGlobal('fetch', fake.fetch)
    expect((await cardsApi.search({ text: 'island' })).cards).toHaveLength(1)
    expect(fake.calls[0]?.url).toBe('/api/cards?q=island')
  })
})

describe('answers that do not match the types', () => {
  const page = searchPage([cardHit('Lightning Bolt')])

  it.each([
    ['an array', []],
    ['no cards field', { total: 0, page: 1, per_page: 40 }],
    ['cards that are not a list', { ...page, cards: 'none' }],
    ['a total that is negative', { ...page, total: -1 }],
    ['a page that is fractional', { ...page, page: 1.5 }],
    ['no page size', { ...page, per_page: undefined }],
    ['a card without a name', { ...page, cards: [{ oracle_id: 'x' }] }],
    [
      'a card with an unknown support state',
      { ...page, cards: [cardHit('X', { support: 'great' as never })] },
    ],
  ])('refuses a search page with %s', async (_what, body) => {
    const { api } = serve(() => jsonResponse(200, body))
    await expect(api.search({ text: 'x' })).rejects.toMatchObject({ code: 'bad_response', status: 200 })
  })

  const detail = cardDetail()
  it.each([
    ['no card', { ...detail, card: undefined }],
    ['no oracle text', { ...detail, oracle_text: undefined }],
    ['keywords that are numbers', { ...detail, keywords: [1] }],
    ['faces that are not objects', { ...detail, faces: ['front'] }],
    ['legalities with a number in them', { ...detail, legalities: { modern: 1 } }],
    ['no printings', { ...detail, printings: undefined }],
    ['no rulings', { ...detail, rulings: undefined }],
    ['reasons that are not objects', { ...detail, reasons: ['why'] }],
  ])('refuses a card with %s', async (_what, body) => {
    const { api } = serve(() => jsonResponse(200, body))
    await expect(api.get('oracle-lightning-bolt')).rejects.toMatchObject({
      code: 'bad_response',
      status: 200,
    })
  })
})

describe('failures', () => {
  // A `Record` over every code: a code added in Rust stops this file compiling until it has a status here.
  const STATUS: Record<DeckErrorCode, number> = {
    invalid_request: 400,
    invalid_deck: 400,
    unknown_cards: 400,
    not_found: 404,
    deck_limit: 409,
    export_failed: 422,
    unauthenticated: 401,
    forbidden_origin: 403,
    payload_too_large: 413,
    unsupported_media_type: 415,
    decks_unavailable: 503,
    cards_unavailable: 503,
    unavailable: 503,
    internal: 500,
  }

  it.each(DECK_ERROR_CODES)('keeps the code, status and sentence of %s on every call', async (code) => {
    for (const [name, run] of CALLS) {
      const { api } = serve(() => deckError(STATUS[code], code, `because of ${name}`))
      const failed = await run(api).then(
        () => null,
        (error: unknown) => error,
      )
      expect(failed, name).toBeInstanceOf(CardsError)
      expect(failed, name).toMatchObject({ code, status: STATUS[code], message: `because of ${name}` })
    }
  })

  it('names every code the table above covers', () => {
    expect([...DECK_ERROR_CODES].sort()).toEqual(Object.keys(STATUS).sort())
  })

  it('reports a server without a card database as 503 cards_unavailable', async () => {
    const { api } = serve(() => deckError(503, 'cards_unavailable', 'this server has no card database'))
    const failed = await api.search({ text: 'x' }).then(
      () => null,
      (error: unknown) => error,
    )
    expect(failed).toMatchObject({ code: 'cards_unavailable', status: 503, name: 'CardsError' })
    expect(isCardsUnavailable(failed)).toBe(true)
  })

  it('tells a malformed filter from an unknown card', async () => {
    const bad = serve(() => deckError(400, 'invalid_request', 'cmc: not a mana value'))
    await expect(bad.api.search({ manaValue: 'x' })).rejects.toMatchObject({
      code: 'invalid_request',
      status: 400,
    })
    const missing = serve(() => deckError(404, 'not_found', 'no such card'))
    await expect(missing.api.get('nope')).rejects.toMatchObject({ code: 'not_found', status: 404 })
  })

  it('calls a code it does not know unknown and keeps the sentence', async () => {
    const { api } = serve(() => jsonResponse(418, { error: 'teapot', message: 'short and stout' }))
    await expect(api.search({})).rejects.toMatchObject({
      code: 'unknown',
      status: 418,
      message: 'short and stout',
    })
  })

  it.each([
    ['an HTML page from a proxy', () => new Response('<html>Bad gateway</html>', { status: 502 })],
    ['an empty body', () => new Response('', { status: 500 })],
    ['JSON without a message', () => jsonResponse(500, { error: 'internal' })],
    ['JSON that is not an object', () => jsonResponse(500, ['internal'])],
  ])('calls %s a bad_response and keeps the status', async (_what, answer) => {
    const { api } = serve(answer)
    await expect(api.search({})).rejects.toMatchObject({ code: 'bad_response', status: answer().status })
  })

  it('calls a request that never got an answer a network failure with status 0', async () => {
    const api = createCardsApi({ fetch: () => Promise.reject(new TypeError('Failed to fetch')) })
    for (const [, run] of CALLS) {
      await expect(run(api)).rejects.toMatchObject({ code: 'network', status: 0 })
    }
  })

  it('calls an answer that breaks off while it is read a network failure', async () => {
    const broken = Object.assign(new Response('x', { status: 200 }), {
      text: () => Promise.reject(new TypeError('connection reset')),
    })
    const api = createCardsApi({ fetch: () => Promise.resolve(broken) })
    await expect(api.search({})).rejects.toMatchObject({ code: 'network', status: 0 })
  })

  it('is not fooled by an error that is not a CardsError', () => {
    expect(isCardsUnavailable(new Error('cards_unavailable'))).toBe(false)
    expect(isCardsUnavailable(undefined)).toBe(false)
  })
})
