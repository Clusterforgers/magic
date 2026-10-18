// Goal: the REST client sends the documented requests, returns only checked bodies and turns every failure into an ApiError with a stable code.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { fakeFetch, jsonResponse } from '@/test-utils/fakeFetch'
import { ApiError, createApi, SERVER_ERROR_CODES } from './api'
import type { Api, ApiErrorCode, CreateGameRequest } from './api'

const request: CreateGameRequest = {
  format: 'normal',
  seats: [
    { name: 'Ana', deck: 'mono-red' },
    { name: 'Bo', deck: 'mono-green' },
  ],
}
const created = {
  game: '00000000-0000-4000-8000-000000000001',
  seats: [
    { seat: 0, token: 'token-0' },
    { seat: 1, token: 'token-1' },
  ],
}
const game = {
  started: true,
  seats: [
    { seat: 0, name: 'Ana' },
    { seat: 1, name: 'Bo' },
  ],
  connected: [0],
  ready: [0],
  result: null,
}
const decks = [
  {
    id: 'mono-red',
    name: 'Mono Red',
    color: 'Red',
    size: 60,
    cards: [
      { name: 'Mountain', count: 24 },
      { name: 'Goblin Piker', count: 4 },
    ],
  },
]

/** The three calls, each with a body the server would send for it. */
const calls: [string, (api: Api) => Promise<unknown>, number, unknown][] = [
  ['createGame', (api) => api.createGame(request), 201, created],
  ['getGame', (api) => api.getGame('g-1'), 200, game],
  ['listDecks', (api) => api.listDecks(), 200, decks],
]

/** Every error `mtg-server` sends (crates/mtg-server/src/api.rs), with its status and a real message. */
const serverErrors: [number, ApiErrorCode, string][] = [
  [400, 'invalid_request', 'a game needs 2 to 12 seats, got 1'],
  [400, 'unsupported_format', 'the only supported format is "normal"'],
  [400, 'unknown_deck', 'seat 0: no such deck; see GET /api/decks'],
  [403, 'forbidden_origin', 'origin not allowed'],
  [404, 'not_found', 'no such resource'],
  [405, 'method_not_allowed', 'method not allowed'],
  [413, 'payload_too_large', 'the request body is limited to 16384 bytes'],
  [415, 'unsupported_media_type', 'send the body as application/json'],
  [500, 'internal', 'the game could not be created'],
  [503, 'server_full', 'the server is running its maximum number of games; try again later'],
  [503, 'unavailable', 'the server is shutting down'],
]

async function failureOf(promise: Promise<unknown>): Promise<ApiError> {
  try {
    await promise
  } catch (error) {
    if (error instanceof ApiError) return error
    throw error
  }
  throw new Error('the call succeeded but should have failed')
}

afterEach(() => vi.unstubAllGlobals())

describe('requests and answers', () => {
  it('posts the game as JSON and returns the tokens of a 201', async () => {
    const fake = fakeFetch(() => jsonResponse(201, created))
    const result = await createApi({ fetch: fake.fetch }).createGame(request)
    expect(result).toEqual(created)
    expect(fake.calls).toHaveLength(1)
    const [call] = fake.calls
    expect(call?.url).toBe('/api/games')
    expect(call?.init?.method).toBe('POST')
    expect(call?.init?.headers).toEqual({ 'content-type': 'application/json' })
    expect(JSON.parse(String(call?.init?.body))).toEqual(request)
  })

  it('sends the session cookie with createGame, which is what binds the creator’s seat to their account', async () => {
    const fake = fakeFetch(() => jsonResponse(201, created))
    await createApi({ fetch: fake.fetch }).createGame(request)
    expect(fake.calls[0]?.init?.credentials).toBe('include')
  })

  it.each<[string, unknown]>([
    ['not started, no result', { ...game, started: false, connected: [], ready: [] }],
    [
      'not started, with ready seats that are away',
      { ...game, started: false, connected: [], ready: [0, 1] },
    ],
    ['a draw', { ...game, result: 'Draw' }],
    ['a winner', { ...game, result: { Winner: 1 }, connected: [0, 1], ready: [0, 1] }],
  ])('reads a game that is %s', async (_name, body) => {
    const fake = fakeFetch(() => jsonResponse(200, body))
    expect(await createApi({ fetch: fake.fetch }).getGame('g-1')).toEqual(body)
    expect(fake.calls[0]?.init).toBeUndefined()
  })

  it('takes a server that sends no ready list for one where nobody is known to be ready', async () => {
    const { ready: _ready, ...older } = game
    const fake = fakeFetch(() => jsonResponse(200, older))
    expect(await createApi({ fetch: fake.fetch }).getGame('g-1')).toEqual({ ...older, ready: [] })
  })

  it('escapes the game id in the path', async () => {
    const fake = fakeFetch(() => jsonResponse(200, game))
    await createApi({ fetch: fake.fetch }).getGame('a/b?c')
    expect(fake.calls[0]?.url).toBe('/api/games/a%2Fb%3Fc')
  })

  it('lists the decks', async () => {
    const fake = fakeFetch(() => jsonResponse(200, decks))
    expect(await createApi({ fetch: fake.fetch }).listDecks()).toEqual(decks)
    expect(fake.calls[0]?.url).toBe('/api/decks')
  })

  it('drops fields it does not know and keeps the ones it does', async () => {
    const fake = fakeFetch(() =>
      jsonResponse(201, { ...created, later: true, seats: [{ ...created.seats[0], x: 1 }] }),
    )
    expect(await createApi({ fetch: fake.fetch }).createGame(request)).toEqual({
      game: created.game,
      seats: [created.seats[0]],
    })
  })

  it('prefixes every path with the base URL', async () => {
    const fake = fakeFetch(() => jsonResponse(200, decks))
    await createApi({ fetch: fake.fetch, baseUrl: 'http://server:8080' }).listDecks()
    expect(fake.calls[0]?.url).toBe('http://server:8080/api/decks')
  })

  it('looks the global fetch up when a call is made, not when the client is built', async () => {
    const client = createApi()
    const fake = fakeFetch(() => jsonResponse(200, decks))
    vi.stubGlobal('fetch', fake.fetch)
    expect(await client.listDecks()).toEqual(decks)
    expect(fake.calls).toHaveLength(1)
  })
})

describe.each(calls)('%s fails with an ApiError', (_name, run, successStatus) => {
  it.each(serverErrors)('on HTTP %i %s, keeping the server message', async (status, code, message) => {
    const fake = fakeFetch(() => jsonResponse(status, { error: code, message }))
    const error = await failureOf(run(createApi({ fetch: fake.fetch })))
    expect(error).toBeInstanceOf(Error)
    expect(error.name).toBe('ApiError')
    expect({ code: error.code, status: error.status, message: error.message }).toEqual({
      code,
      status,
      message,
    })
  })

  it('with code unknown when the server sends a code this client has not heard of', async () => {
    const fake = fakeFetch(() => jsonResponse(418, { error: 'teapot', message: 'short and stout' }))
    const error = await failureOf(run(createApi({ fetch: fake.fetch })))
    expect({ code: error.code, status: error.status, message: error.message }).toEqual({
      code: 'unknown',
      status: 418,
      message: 'short and stout',
    })
  })

  it.each<[string, () => Promise<Response>]>([
    ['the request is refused outright', () => Promise.reject(new TypeError('Failed to fetch'))],
    ['it throws something that is not an error', () => Promise.reject('offline')],
    [
      'the body breaks off',
      () =>
        Promise.resolve(
          Object.assign(jsonResponse(200, {}), { text: () => Promise.reject(new Error('reset')) }),
        ),
    ],
  ])('with code network and status 0 when %s', async (_what, answer) => {
    const fake = fakeFetch(answer)
    const error = await failureOf(run(createApi({ fetch: fake.fetch })))
    expect(error.code).toBe('network')
    expect(error.status).toBe(0)
    expect(error.message).toMatch(/could not reach the server/i)
  })

  it.each<[string, number, string]>([
    ['an HTML page on success', successStatus, '<html><body>welcome</body></html>'],
    ['an empty body on success', successStatus, ''],
    ['JSON null on success', successStatus, 'null'],
    ['an HTML gateway error', 502, '<html><body>Bad Gateway</body></html>'],
    ['an empty gateway timeout', 504, ''],
    ['an error object without a message', 400, '{"error":"invalid_request"}'],
    ['an error whose code is not text', 400, '{"error":7,"message":"seven"}'],
    ['a list as the error', 500, '[]'],
    ['plain text as the error', 503, 'service unavailable'],
  ])('with code bad_response and the status kept on %s', async (_what, status, text) => {
    const fake = fakeFetch(() => new Response(text, { status }))
    const error = await failureOf(run(createApi({ fetch: fake.fetch })))
    expect(error.code).toBe('bad_response')
    expect(error.status).toBe(status)
  })
})

describe('bodies that parse but do not have the documented shape', () => {
  const wrong: [string, (api: Api) => Promise<unknown>, unknown][] = [
    ['createGame: game is a number', (api) => api.createGame(request), { ...created, game: 5 }],
    ['createGame: seats is text', (api) => api.createGame(request), { ...created, seats: 'two' }],
    [
      'createGame: a seat has no token',
      (api) => api.createGame(request),
      { ...created, seats: [{ seat: 0 }] },
    ],
    [
      'createGame: a seat number is negative',
      (api) => api.createGame(request),
      { ...created, seats: [{ seat: -1, token: 't' }] },
    ],
    ['getGame: started is text', (api) => api.getGame('g'), { ...game, started: 'yes' }],
    ['getGame: connected has a fraction', (api) => api.getGame('g'), { ...game, connected: [0.5] }],
    ['getGame: ready has a fraction', (api) => api.getGame('g'), { ...game, ready: [0.5] }],
    ['getGame: ready is a number', (api) => api.getGame('g'), { ...game, ready: 1 }],
    ['getGame: ready is null', (api) => api.getGame('g'), { ...game, ready: null }],
    ['getGame: a ready seat is negative', (api) => api.getGame('g'), { ...game, ready: [-1] }],
    ['getGame: result is an unknown word', (api) => api.getGame('g'), { ...game, result: 'Win' }],
    ['getGame: result is missing', (api) => api.getGame('g'), { started: true, seats: [], connected: [] }],
    ['getGame: a seat has no name', (api) => api.getGame('g'), { ...game, seats: [{ seat: 0 }] }],
    ['listDecks: an object, not a list', (api) => api.listDecks(), { decks }],
    ['listDecks: a deck has no id', (api) => api.listDecks(), [{ ...decks[0], id: undefined }]],
    ['listDecks: an unknown colour', (api) => api.listDecks(), [{ ...decks[0], color: 'Purple' }]],
    [
      'listDecks: a card line without a count',
      (api) => api.listDecks(),
      [{ ...decks[0], cards: [{ name: 'Mountain' }] }],
    ],
  ]

  it.each(wrong)('%s is a bad_response, not a half-typed value', async (_name, run, body) => {
    const fake = fakeFetch(() => jsonResponse(200, body))
    const error = await failureOf(run(createApi({ fetch: fake.fetch })))
    expect(error.code).toBe('bad_response')
    expect(error.status).toBe(200)
  })
})

describe('the error table', () => {
  it('covers every code the client recognises, once', () => {
    const codes = serverErrors.map(([, code]) => code)
    expect(new Set(codes)).toEqual(new Set(SERVER_ERROR_CODES))
    expect(new Set(codes).size).toBe(codes.length)
  })
})
