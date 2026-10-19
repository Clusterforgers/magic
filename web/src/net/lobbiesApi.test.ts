// Goal: the game browser calls send the documented requests (path, method, body, token and session), keep the list's ETag and answer a 304 with the listing already held, return only checked bodies, and turn every failure into a LobbyError whose code, status and wait are the server's.
import { describe, expect, it } from 'vitest'
import { fakeFetch, jsonResponse } from '@/test-utils/fakeFetch'
import type { FetchCall } from '@/test-utils/fakeFetch'
import { createApi } from './api'
import type { Api, LobbyListing } from './api'
import { LOBBY_ERROR_CODES, LobbyError } from './lobbyErrors'

const GAME = '6f1e0c1a-3b1f-4d9e-8a3e-0d3c2d1b7a55'
const TOKEN = 'a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90'

const row = {
  game: GAME,
  host: 'Ann',
  format: 'normal',
  filled: 1,
  total: 4,
  created_at: '2026-10-19T12:30:00Z',
  age_secs: 125,
}

function empty(status: number, headers: Record<string, string> = {}): Response {
  return new Response(null, { status, headers })
}

function listing(over: Partial<LobbyListing> = {}): LobbyListing {
  return { games: [row], etag: '"abc"', fetchedAt: 1_000, ...over }
}

async function failureOf(promise: Promise<unknown>): Promise<LobbyError> {
  try {
    await promise
  } catch (error) {
    if (error instanceof LobbyError) return error
    throw error
  }
  throw new Error('the call succeeded but should have failed')
}

/** Every call of the game browser, with a way to make it and an answer the server gives it on success. */
const calls: [string, (api: Api) => Promise<unknown>, () => Response][] = [
  ['listLobbies', (api) => api.listLobbies(), () => jsonResponse(200, { games: [row] })],
  ['joinGame', (api) => api.joinGame(GAME, { name: 'Bo' }), () => jsonResponse(200, { seat: 1, token: 't' })],
  [
    'joinByCode',
    (api) => api.joinByCode('ABCD2345', { name: 'Bo' }),
    () => jsonResponse(200, { seat: 1, token: 't' }),
  ],
  ['leaveGame', (api) => api.leaveGame(GAME, { token: TOKEN }), () => empty(204)],
  ['releaseSeat', (api) => api.releaseSeat(GAME, 1, { token: TOKEN }), () => empty(204)],
  [
    'setVisibility',
    (api) => api.setVisibility(GAME, 'invite', { token: TOKEN }),
    () => jsonResponse(200, { visibility: 'invite' }),
  ],
]

/** Every error the game browser endpoints send, with its status and a real message. */
const serverErrors: [number, string, string][] = [
  [400, 'invalid_request', 'the body is not a valid request: missing field `code`'],
  [400, 'invalid_name', 'the name must be 1 to 32 characters without control characters'],
  [401, 'unauthenticated', 'sign in to do that'],
  [403, 'forbidden_origin', 'origin not allowed'],
  [403, 'forbidden', 'only the host can do that'],
  [404, 'not_found', 'no such resource'],
  [409, 'full', 'every seat of that game is taken'],
  [409, 'started', 'that game has already started'],
  [409, 'not_claimed', 'that seat was made with the game'],
  [413, 'payload_too_large', 'the request body is limited to 16384 bytes'],
  [415, 'unsupported_media_type', 'send the body as application/json'],
  [429, 'too_many_attempts', 'too many wrong codes; try again in 30 seconds'],
  [429, 'rate_limited', 'too many requests for the list; try again in 30 seconds'],
  [503, 'accounts_unavailable', 'this server runs without a database'],
  [503, 'lobbies_unavailable', 'the server is shutting down'],
  [503, 'unavailable', 'the seat could not be saved; try again later'],
  [500, 'internal', 'the seat could not be claimed'],
]

describe('listLobbies', () => {
  it('asks for the list with no conditions the first time and reads the rows, the tag and the time', async () => {
    const rows = [row, { ...row, game: 'g-2' }]
    const fake = fakeFetch(
      () => new Response(JSON.stringify({ games: rows }), { status: 200, headers: { etag: '"tag-1"' } }),
    )
    const result = await createApi({ fetch: fake.fetch, now: () => 42_000 }).listLobbies()
    expect(result).toEqual({ games: rows, etag: '"tag-1"', fetchedAt: 42_000 })
    expect(fake.calls).toHaveLength(1)
    expect(fake.calls[0]?.url).toBe('/api/lobbies')
    expect(fake.calls[0]?.init).toEqual({})
  })

  it('reads a list the server sent without a tag, and then asks for it unconditionally', async () => {
    const fake = fakeFetch(() => jsonResponse(200, { games: [] }))
    const api = createApi({ fetch: fake.fetch, now: () => 5 })
    const first = await api.listLobbies()
    expect(first).toEqual({ games: [], etag: null, fetchedAt: 5 })
    await api.listLobbies(first)
    expect(fake.calls[1]?.init).toEqual({})
  })

  it('sends the tag it holds as If-None-Match', async () => {
    const fake = fakeFetch(() => empty(304, { etag: '"abc"' }))
    await createApi({ fetch: fake.fetch }).listLobbies(listing())
    expect(fake.calls[0]?.init).toEqual({ headers: { 'if-none-match': '"abc"' } })
  })

  it('answers a 304 with the very listing it holds, rows and time untouched', async () => {
    const held = listing({ fetchedAt: 1_000 })
    const fake = fakeFetch(() => empty(304, { etag: '"abc"' }))
    const result = await createApi({ fetch: fake.fetch, now: () => 99_999 }).listLobbies(held)
    expect(result).toBe(held)
    expect(result.fetchedAt).toBe(1_000)
  })

  it('takes the new rows, tag and time when the list changed', async () => {
    const held = listing()
    const changed = { ...row, filled: 2 }
    const fake = fakeFetch(
      () => new Response(JSON.stringify({ games: [changed] }), { status: 200, headers: { etag: '"def"' } }),
    )
    const result = await createApi({ fetch: fake.fetch, now: () => 7_000 }).listLobbies(held)
    expect(result).not.toBe(held)
    expect(result).toEqual({ games: [changed], etag: '"def"', fetchedAt: 7_000 })
    // The listing that was held is not touched.
    expect(held.games).toEqual([row])
  })

  it.each<[string, LobbyListing | undefined]>([
    ['nothing held', undefined],
    ['a listing with no tag', listing({ etag: null })],
  ])('does not take a 304 when it asked for no condition (%s)', async (_what, held) => {
    const fake = fakeFetch(() => empty(304))
    const error = await failureOf(createApi({ fetch: fake.fetch }).listLobbies(held))
    expect([error.code, error.status]).toEqual(['bad_response', 304])
  })

  it('keeps the wait of a 429 for the caller to show', async () => {
    const fake = fakeFetch(
      () =>
        new Response(JSON.stringify({ error: 'rate_limited', message: 'slow down' }), {
          status: 429,
          headers: { 'retry-after': '12', 'content-type': 'application/json' },
        }),
    )
    const error = await failureOf(createApi({ fetch: fake.fetch }).listLobbies(listing()))
    expect([error.code, error.status, error.retryAfter]).toEqual(['rate_limited', 429, 12])
  })

  it('looks the clock up as given, not as the global one', async () => {
    const fake = fakeFetch(() => jsonResponse(200, { games: [] }))
    const ticks = [10, 20]
    const api = createApi({ fetch: fake.fetch, now: () => ticks.shift() ?? 0 })
    expect((await api.listLobbies()).fetchedAt).toBe(10)
    expect((await api.listLobbies()).fetchedAt).toBe(20)
  })
})

describe('joinGame and joinByCode', () => {
  it('post the name as JSON with the session cookie and read the seat and its token', async () => {
    const fake = fakeFetch(() => jsonResponse(200, { seat: 2, token: 'secret' }))
    const joined = await createApi({ fetch: fake.fetch }).joinGame(GAME, { name: 'Bo' })
    expect(joined).toEqual({ seat: 2, token: 'secret' })
    const [call] = fake.calls
    expect(call?.url).toBe(`/api/games/${GAME}/join`)
    expect(call?.init).toEqual({
      method: 'POST',
      credentials: 'include',
      headers: { 'content-type': 'application/json' },
      body: '{"name":"Bo"}',
    })
  })

  it('send an empty object for a signed-in visitor who gives no name', async () => {
    const fake = fakeFetch(() => jsonResponse(200, { seat: 1, token: null }))
    const api = createApi({ fetch: fake.fetch })
    expect(await api.joinGame(GAME)).toEqual({ seat: 1, token: null })
    expect(await api.joinByCode('ABCD2345')).toEqual({ seat: 1, token: null })
    expect(fake.calls.map((call) => call.init?.body)).toEqual(['{}', '{"code":"ABCD2345"}'])
  })

  it('sends the code as it was typed and leaves the judging to the server', async () => {
    const fake = fakeFetch(() => jsonResponse(200, { seat: 1, token: 't' }))
    await createApi({ fetch: fake.fetch }).joinByCode(' abcd-2345 ', { name: 'Bo' })
    expect(fake.calls[0]?.url).toBe('/api/join')
    expect(JSON.parse(String(fake.calls[0]?.init?.body))).toEqual({ code: ' abcd-2345 ', name: 'Bo' })
  })

  it('escapes the game id in the path', async () => {
    const fake = fakeFetch(() => jsonResponse(200, { seat: 1, token: 't' }))
    await createApi({ fetch: fake.fetch }).joinGame('a/b?c')
    expect(fake.calls[0]?.url).toBe('/api/games/a%2Fb%3Fc/join')
  })

  it.each<[string, unknown]>([
    ['no token key', { seat: 1 }],
    ['a numeric token', { seat: 1, token: 7 }],
    ['a negative seat', { seat: -1, token: 't' }],
    ['a fractional seat', { seat: 1.5, token: 't' }],
    ['a list', [1, 't']],
  ])('refuse an answer with %s as a bad_response', async (_what, body) => {
    const fake = fakeFetch(() => jsonResponse(200, body))
    const api = createApi({ fetch: fake.fetch })
    for (const error of [
      await failureOf(api.joinGame(GAME, { name: 'Bo' })),
      await failureOf(api.joinByCode('ABCD2345', { name: 'Bo' })),
    ]) {
      expect([error.code, error.status]).toEqual(['bad_response', 200])
    }
  })
})

describe('leaveGame and releaseSeat', () => {
  it('post with the seat token as a bearer token and the session cookie, and resolve with nothing on a 204', async () => {
    const fake = fakeFetch(() => empty(204))
    const api = createApi({ fetch: fake.fetch })
    expect(await api.leaveGame(GAME, { token: TOKEN })).toBeUndefined()
    expect(await api.releaseSeat(GAME, 3, { token: TOKEN })).toBeUndefined()
    const [leave, release] = fake.calls
    expect(leave?.url).toBe(`/api/games/${GAME}/leave`)
    expect(release?.url).toBe(`/api/games/${GAME}/seats/3/release`)
    for (const call of [leave, release]) {
      expect(call?.init).toEqual({
        method: 'POST',
        credentials: 'include',
        headers: { authorization: `Bearer ${TOKEN}` },
      })
    }
  })

  it('rely on the session alone when no token is given', async () => {
    const fake = fakeFetch(() => empty(204))
    const api = createApi({ fetch: fake.fetch })
    await api.leaveGame(GAME)
    await api.releaseSeat(GAME, 1, {})
    for (const call of fake.calls) {
      expect(call.init).toEqual({ method: 'POST', credentials: 'include', headers: {} })
    }
  })

  it.each<[string, number]>([
    ['negative', -1],
    ['a fraction', 1.5],
    ['not a number', Number.NaN],
    ['infinite', Infinity],
  ])('does not send a seat number that is %s', async (_what, seat) => {
    const fake = fakeFetch(() => empty(204))
    const error = await failureOf(createApi({ fetch: fake.fetch }).releaseSeat(GAME, seat, { token: TOKEN }))
    expect([error.code, error.status]).toEqual(['invalid_request', 0])
    expect(fake.calls).toEqual([])
  })

  it.each<[string, string]>([
    ['a space', 'has space'],
    ['a newline', 'line\nbreak'],
    ['a non-ASCII letter', 'tokén'],
    ['nothing', ''],
  ])('does not put a token with %s into a header', async (_what, token) => {
    const fake = fakeFetch(() => empty(204))
    const api = createApi({ fetch: fake.fetch })
    for (const run of [
      () => api.leaveGame(GAME, { token }),
      () => api.releaseSeat(GAME, 1, { token }),
      () => api.setVisibility(GAME, 'invite', { token }),
    ]) {
      const error = await failureOf(run())
      expect([error.code, error.status]).toEqual(['invalid_request', 0])
    }
    expect(fake.calls).toEqual([])
  })

  it.each<[string, () => Response]>([
    ['a 200 with a body', () => jsonResponse(200, { ok: true })],
    ['a 200 with nothing', () => empty(200)],
    ['an HTML page', () => new Response('<html>ok</html>', { status: 200 })],
  ])('call %s a bad_response, since the server answers 204', async (_what, answer) => {
    const fake = fakeFetch(answer)
    const api = createApi({ fetch: fake.fetch })
    for (const run of [
      () => api.leaveGame(GAME, { token: TOKEN }),
      () => api.releaseSeat(GAME, 1, { token: TOKEN }),
    ]) {
      const error = await failureOf(run())
      expect([error.code, error.status]).toEqual(['bad_response', 200])
    }
  })
})

describe('setVisibility', () => {
  it('patches the visibility as JSON with the token and the cookie and returns the visibility in force', async () => {
    const fake = fakeFetch(() => jsonResponse(200, { visibility: 'invite' }))
    const result = await createApi({ fetch: fake.fetch }).setVisibility(GAME, 'invite', { token: TOKEN })
    expect(result).toBe('invite')
    expect(fake.calls[0]?.url).toBe(`/api/games/${GAME}`)
    expect(fake.calls[0]?.init).toEqual({
      method: 'PATCH',
      credentials: 'include',
      headers: { 'content-type': 'application/json', authorization: `Bearer ${TOKEN}` },
      body: '{"visibility":"invite"}',
    })
  })

  it('works through the session alone', async () => {
    const fake = fakeFetch(() => jsonResponse(200, { visibility: 'public' }))
    await createApi({ fetch: fake.fetch }).setVisibility(GAME, 'public')
    expect(fake.calls[0]?.init?.headers).toEqual({ 'content-type': 'application/json' })
  })

  it.each<[string, unknown]>([
    ['another word', { visibility: 'private' }],
    ['no field', {}],
    ['null', null],
  ])('refuses an answer with %s as a bad_response', async (_what, body) => {
    const fake = fakeFetch(() => jsonResponse(200, body))
    const error = await failureOf(createApi({ fetch: fake.fetch }).setVisibility(GAME, 'public'))
    expect(error.code).toBe('bad_response')
  })
})

describe.each(calls)('%s fails with a LobbyError', (_name, run, success) => {
  it.each(serverErrors)('on HTTP %i %s, keeping the server message', async (status, code, message) => {
    const fake = fakeFetch(() => jsonResponse(status, { error: code, message }))
    const error = await failureOf(run(createApi({ fetch: fake.fetch })))
    expect(error).toBeInstanceOf(Error)
    expect(error.name).toBe('LobbyError')
    expect({ code: error.code, status: error.status, message: error.message }).toEqual({
      code,
      status,
      message,
    })
    expect(error.retryAfter).toBeNull()
  })

  it('keeps the Retry-After of a 429 as seconds', async () => {
    const fake = fakeFetch(
      () =>
        new Response(JSON.stringify({ error: 'too_many_attempts', message: 'wait' }), {
          status: 429,
          headers: { 'retry-after': '41' },
        }),
    )
    const error = await failureOf(run(createApi({ fetch: fake.fetch })))
    expect([error.code, error.retryAfter]).toEqual(['too_many_attempts', 41])
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
      () => Promise.resolve(Object.assign(success(), { text: () => Promise.reject(new Error('reset')) })),
    ],
  ])('with code network and status 0 when %s', async (_what, answer) => {
    const fake = fakeFetch(answer)
    const error = await failureOf(run(createApi({ fetch: fake.fetch })))
    expect([error.code, error.status]).toEqual(['network', 0])
    expect(error.message).toMatch(/could not reach the server/i)
  })

  it.each<[string, number, string]>([
    ['an HTML gateway error', 502, '<html><body>Bad Gateway</body></html>'],
    ['an empty gateway timeout', 504, ''],
    ['an error object without a message', 400, '{"error":"invalid_request"}'],
    ['an error whose code is not text', 400, '{"error":7,"message":"seven"}'],
    ['a list as the error', 500, '[]'],
    ['plain text as the error', 503, 'service unavailable'],
  ])('with code bad_response and the status kept on %s', async (_what, status, text) => {
    const fake = fakeFetch(() => new Response(text, { status }))
    const error = await failureOf(run(createApi({ fetch: fake.fetch })))
    expect([error.code, error.status]).toEqual(['bad_response', status])
  })

  it('prefixes the path with the base URL', async () => {
    const fake = fakeFetch(() => success())
    await run(createApi({ fetch: fake.fetch, baseUrl: 'http://server:8080' }))
    expect(fake.calls[0]?.url).toMatch(/^http:\/\/server:8080\/api\//)
  })
})

// The calls whose success has a body to read (the others answer 204 and have none to get wrong).
describe.each(calls.filter(([, , success]) => success().status === 200))(
  '%s reads its answer',
  (_name, run) => {
    it.each<[string, string]>([
      ['an HTML page', '<html><body>welcome</body></html>'],
      ['nothing', ''],
      ['JSON null', 'null'],
    ])('and calls %s a bad_response', async (_what, text) => {
      const fake = fakeFetch(() => new Response(text, { status: 200 }))
      const error = await failureOf(run(createApi({ fetch: fake.fetch })))
      expect([error.code, error.status]).toEqual(['bad_response', 200])
    })
  },
)

describe('the error table', () => {
  it('covers every code the client recognises, once', () => {
    const codes = serverErrors.map(([, code]) => code)
    expect(new Set(codes)).toEqual(new Set(LOBBY_ERROR_CODES))
    expect(new Set(codes).size).toBe(codes.length)
  })
})

describe('what the calls leave alone', () => {
  it('make no request when the page only builds the client', () => {
    const fake = fakeFetch(() => empty(204))
    createApi({ fetch: fake.fetch })
    expect(fake.calls).toEqual<FetchCall[]>([])
  })
})
