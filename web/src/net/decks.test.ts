// Goal: the deck client sends each call to its endpoint with the session cookie and the right body, returns only answers that match the generated types, and turns every failure into a DeckError whose code is the server's DeckErrorCode or a local unknown, network or bad_response, 503 decks_unavailable included.
import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  cardHit,
  counts,
  deckError,
  deckItem,
  deckView,
  importResult,
  lineOf,
  summary,
} from '@/features/decks/fakeDecks'
import type { DeckBody, DeckErrorCode, ImportRequest } from '@/protocol/generated'
import { fakeFetch, jsonResponse } from '@/test-utils/fakeFetch'
import type { FetchCall } from '@/test-utils/fakeFetch'
import { createDecksApi, DECK_ERROR_CODES, DeckError, decksApi, isDeckErrorCode } from './decks'
import type { DecksApi } from './decks'

afterEach(() => {
  vi.unstubAllGlobals()
})

const BODY: DeckBody = {
  name: 'Burn',
  format: 'modern',
  notes: '',
  cards: [{ zone: 'main', oracle_id: 'oracle-lightning-bolt', printing_id: null, count: 4 }],
}
const REQUEST: ImportRequest = { text: '4 Lightning Bolt', source: null, format: 'modern' }

/** The client over a fetch that answers every request with `answer`. */
function serve(answer: (call: FetchCall) => Response | Promise<Response>, baseUrl?: string) {
  const fake = fakeFetch(answer)
  return { api: createDecksApi({ fetch: fake.fetch, baseUrl }), calls: fake.calls }
}

const CALLS: ReadonlyArray<readonly [string, (api: DecksApi) => Promise<unknown>]> = [
  ['list', (api) => api.list()],
  ['get', (api) => api.get('deck-1')],
  ['create', (api) => api.create(BODY)],
  ['importList', (api) => api.importList(REQUEST)],
  ['remove', (api) => api.remove('deck-1')],
]

describe('the requests', () => {
  it('lists with the cookie and no body', async () => {
    const { api, calls } = serve(() => jsonResponse(200, { decks: [deckItem(), deckItem({ id: 'deck-2' })] }))
    const decks = await api.list()
    expect(decks.map((deck) => deck.id)).toEqual(['deck-1', 'deck-2'])
    expect(calls).toHaveLength(1)
    expect(calls[0]?.url).toBe('/api/decks')
    expect(calls[0]?.init).toEqual({ credentials: 'include' })
  })

  it('reads one deck by an id that is escaped into the path', async () => {
    const { api, calls } = serve(() => jsonResponse(200, deckView({ id: 'a b/c' })))
    expect((await api.get('a b/c')).id).toBe('a b/c')
    expect(calls[0]?.url).toBe('/api/decks/a%20b%2Fc')
    expect(calls[0]?.init?.credentials).toBe('include')
  })

  it('saves a deck as JSON and returns what the server stored', async () => {
    const { api, calls } = serve(() => jsonResponse(201, deckView({ name: 'Burn', state: 'manual_play' })))
    const saved = await api.create(BODY)
    expect(saved).toMatchObject({ name: 'Burn', state: 'manual_play' })
    expect(calls[0]?.url).toBe('/api/decks')
    expect(calls[0]?.init?.method).toBe('POST')
    expect(calls[0]?.init?.credentials).toBe('include')
    expect(calls[0]?.init?.headers).toEqual({ 'content-type': 'application/json' })
    expect(JSON.parse(String(calls[0]?.init?.body))).toEqual(BODY)
  })

  it('sends a pasted list to the import endpoint and returns what it read', async () => {
    const read = importResult({
      ignored: [{ name: 'Maybeboard', count: 12 }],
      unresolved: [
        {
          name: 'Lightening Bolt',
          count: 2,
          zone: 'main',
          reason: 'unknown_card',
          message: 'no card has this name',
          suggestions: [{ oracle_id: 'oracle-lightning-bolt', name: 'Lightning Bolt' }],
        },
      ],
      problems: [{ line: 3, message: 'no count' }],
    })
    const { api, calls } = serve(() => jsonResponse(200, read))
    expect(await api.importList(REQUEST)).toEqual(read)
    expect(calls[0]?.url).toBe('/api/decks/import')
    expect(calls[0]?.init?.method).toBe('POST')
    expect(JSON.parse(String(calls[0]?.init?.body))).toEqual(REQUEST)
  })

  it('deletes with DELETE and accepts the empty 204', async () => {
    const { api, calls } = serve(() => new Response(null, { status: 204 }))
    await expect(api.remove('deck-1')).resolves.toBeUndefined()
    expect(calls[0]?.url).toBe('/api/decks/deck-1')
    expect(calls[0]?.init).toEqual({ method: 'DELETE', credentials: 'include' })
  })

  it('puts the base address in front of every path', async () => {
    const { api, calls } = serve(() => jsonResponse(200, { decks: [] }), 'http://game.test')
    await api.list()
    expect(calls[0]?.url).toBe('http://game.test/api/decks')
    expect(api.exportUrl('deck-1', 'mtgo')).toBe('http://game.test/api/decks/deck-1/export?format=mtgo')
  })

  it('uses the global fetch of the moment for the shared client', async () => {
    const fake = fakeFetch(() => jsonResponse(200, { decks: [deckItem()] }))
    vi.stubGlobal('fetch', fake.fetch)
    expect(await decksApi.list()).toHaveLength(1)
    expect(fake.calls[0]?.url).toBe('/api/decks')
  })
})

describe('the export address', () => {
  const { api } = serve(() => jsonResponse(200, {}))

  it.each([
    [undefined, 'text'],
    ['text', 'text'],
    ['arena', 'arena'],
    ['mtgo', 'mtgo'],
  ] as const)('for %s asks for format=%s', (format, asked) => {
    expect(api.exportUrl('deck-1', format)).toBe(`/api/decks/deck-1/export?format=${asked}`)
  })

  it('escapes the id', () => {
    expect(api.exportUrl('a/b?c', 'arena')).toBe('/api/decks/a%2Fb%3Fc/export?format=arena')
  })
})

describe('answers that do not match the types', () => {
  const item = deckItem()
  const noCounts = { ...summary(), counts: { ...counts(), total: -1 } }

  it.each([
    ['an array', []],
    ['no decks field', {}],
    ['decks that is not a list', { decks: 'none' }],
    ['a deck with a number for an id', { decks: [{ ...item, id: 7 }] }],
    ['a deck in a state the server never sends', { decks: [{ ...item, state: 'archived' }] }],
    ['a deck without its check time', { decks: [{ ...item, checked_at: undefined }] }],
    ['a deck whose counts are negative', { decks: [{ ...item, summary: noCounts }] }],
    [
      'a deck whose summary zone is unknown',
      { decks: [{ ...item, summary: { ...summary(), zones: [{ zone: 'x', counts: counts() }] } }] },
    ],
  ])('refuses a list with %s', async (_what, body) => {
    const { api } = serve(() => jsonResponse(200, body))
    await expect(api.list()).rejects.toMatchObject({ code: 'bad_response', status: 200 })
  })

  it.each([
    ['no report', { ...deckView(), report: undefined }],
    ['a report without a summary', { ...deckView(), report: {} }],
    ['a state the server never sends', { ...deckView(), state: 'great' }],
    ['lines that are not objects', { ...deckView(), cards: ['Lightning Bolt'] }],
    ['a check that is neither null nor an object', { ...deckView(), check: 'ok' }],
  ])('refuses a deck with %s', async (_what, body) => {
    const { api } = serve(() => jsonResponse(200, body))
    await expect(api.get('deck-1')).rejects.toMatchObject({ code: 'bad_response' })
    await expect(api.create(BODY)).rejects.toMatchObject({ code: 'bad_response' })
  })

  const line = lineOf('Lightning Bolt', 4)
  it.each([
    ['a source that is not one of the six', { ...importResult(), source: 'carrier pigeon' }],
    ['a line in a zone that does not exist', { ...importResult(), cards: [{ ...line, zone: 'graveyard' }] }],
    ['a line with a fractional count', { ...importResult(), cards: [{ ...line, count: 1.5 }] }],
    ['a card without a name', { ...importResult(), cards: [{ ...line, card: { oracle_id: 'x' } }] }],
    [
      'a card with an unknown support state',
      { ...importResult(), cards: [{ ...line, card: cardHit('X', { support: 'great' as never }) }] },
    ],
    [
      'an unresolved line with an unknown reason',
      {
        ...importResult(),
        unresolved: [{ name: 'X', count: 1, zone: 'main', reason: 'why', message: 'm', suggestions: [] }],
      },
    ],
    [
      'a suggestion without a name',
      {
        ...importResult(),
        unresolved: [
          {
            name: 'X',
            count: 1,
            zone: 'main',
            reason: 'unknown_card',
            message: 'm',
            suggestions: [{ oracle_id: 'x' }],
          },
        ],
      },
    ],
    [
      'a problem whose line is not a number',
      { ...importResult(), problems: [{ line: 'one', message: 'm' }] },
    ],
    ['an ignored part without a count', { ...importResult(), ignored: [{ name: 'Maybeboard' }] }],
    ['no ignored list', { ...importResult(), ignored: undefined }],
  ])('refuses an import with %s', async (_what, body) => {
    const { api } = serve(() => jsonResponse(200, body))
    await expect(api.importList(REQUEST)).rejects.toMatchObject({ code: 'bad_response', status: 200 })
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
      expect(failed, name).toBeInstanceOf(DeckError)
      expect(failed, name).toMatchObject({ code, status: STATUS[code], message: `because of ${name}` })
    }
  })

  it('names every code the table above covers', () => {
    expect([...DECK_ERROR_CODES].sort()).toEqual(Object.keys(STATUS).sort())
  })

  it('reports a server without a database as 503 decks_unavailable', async () => {
    const { api } = serve(() => deckError(503, 'decks_unavailable', 'this server has no database'))
    const failed = await api.list().then(
      () => null,
      (error: unknown) => error,
    )
    expect(failed).toBeInstanceOf(DeckError)
    expect(failed).toMatchObject({ code: 'decks_unavailable', status: 503, name: 'DeckError' })
  })

  it('calls a code it does not know unknown and keeps the sentence', async () => {
    const { api } = serve(() => jsonResponse(418, { error: 'teapot', message: 'short and stout' }))
    await expect(api.list()).rejects.toMatchObject({
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
    await expect(api.remove('deck-1')).rejects.toMatchObject({ code: 'bad_response' })
    await expect(api.remove('deck-1')).rejects.toMatchObject({ status: answer().status })
  })

  it('calls a request that never got an answer a network failure with status 0', async () => {
    const api = createDecksApi({ fetch: () => Promise.reject(new TypeError('Failed to fetch')) })
    for (const [, run] of CALLS) {
      await expect(run(api)).rejects.toMatchObject({ code: 'network', status: 0 })
    }
  })

  it('calls an answer that breaks off while it is read a network failure', async () => {
    const broken = Object.assign(new Response('x', { status: 200 }), {
      text: () => Promise.reject(new TypeError('connection reset')),
    })
    const api = createDecksApi({ fetch: () => Promise.resolve(broken) })
    await expect(api.list()).rejects.toMatchObject({ code: 'network', status: 0 })
  })
})

describe('isDeckErrorCode', () => {
  it.each(DECK_ERROR_CODES)('accepts %s', (code) => {
    expect(isDeckErrorCode(code)).toBe(true)
  })

  it.each(['', 'teapot', 'toString', '__proto__', 'constructor', 'Not_Found'])('refuses %j', (code) => {
    expect(isDeckErrorCode(code)).toBe(false)
  })
})
