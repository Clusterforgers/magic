// Goal: the accounts client sends the documented requests with the session cookie on every one, returns only checked bodies and turns every failure into an AccountError with a stable code and the server's wait.
import { describe, expect, it } from 'vitest'
import { fakeFetch, jsonResponse } from '@/test-utils/fakeFetch'
import { authError, listed } from '@/test-utils/accountsServer'
import { AccountError, AUTH_ERROR_CODES, createAccountsApi, isAuthErrorCode } from './accounts'
import type { AccountsApi } from './accounts'

const credentials = { username: 'ada_l', password: 'correct horse battery' }
const user = { id: 'u-1', username: 'ada_l' }
const finished = listed({
  status: 'finished',
  result: { Winner: 0 },
  others: [{ seat: 1, name: 'Bo', user: 'bo' }],
})

/** Every call, with a body the server would send for it and the request it must make. */
const calls: [string, (api: AccountsApi) => Promise<unknown>, number, unknown, string, string][] = [
  ['register', (api) => api.register(credentials), 201, user, 'POST', '/api/auth/register'],
  ['login', (api) => api.login(credentials), 200, user, 'POST', '/api/auth/login'],
  ['me', (api) => api.me(), 200, user, 'GET', '/api/auth/me'],
  ['myGames', (api) => api.myGames(), 200, { games: [finished] }, 'GET', '/api/me/games'],
]

describe('requests', () => {
  it.each(calls)('%s sends %s %s with the session cookie', async (_name, run, status, body, method, path) => {
    const fake = fakeFetch(() => jsonResponse(status, body))
    await run(createAccountsApi({ fetch: fake.fetch }))
    expect(fake.calls).toHaveLength(1)
    expect(fake.calls[0]?.url).toBe(path)
    expect(fake.calls[0]?.init?.method ?? 'GET').toBe(method)
    expect(fake.calls[0]?.init?.credentials).toBe('include')
  })

  it.each(['register', 'login'] as const)(
    '%s sends the credentials as JSON and nothing else',
    async (name) => {
      const fake = fakeFetch(() => jsonResponse(200, user))
      await createAccountsApi({ fetch: fake.fetch })[name](credentials)
      expect(fake.calls[0]?.init?.headers).toEqual({ 'content-type': 'application/json' })
      expect(JSON.parse(String(fake.calls[0]?.init?.body))).toEqual(credentials)
    },
  )

  it('signs out with a POST that has no body, and accepts the empty 204', async () => {
    const fake = fakeFetch(() => new Response(null, { status: 204 }))
    await expect(createAccountsApi({ fetch: fake.fetch }).logout()).resolves.toBeUndefined()
    expect(fake.calls[0]?.url).toBe('/api/auth/logout')
    expect(fake.calls[0]?.init).toEqual({ method: 'POST', credentials: 'include' })
  })

  it('removes a failed game with a DELETE that has no body, names the game in the path and accepts the 204', async () => {
    const fake = fakeFetch(() => new Response(null, { status: 204 }))
    await expect(createAccountsApi({ fetch: fake.fetch }).removeGame('g 1/2')).resolves.toBeUndefined()
    expect(fake.calls[0]?.url).toBe('/api/me/games/g%201%2F2')
    expect(fake.calls[0]?.init).toEqual({ method: 'DELETE', credentials: 'include' })
  })

  it('reports a game the server does not hold as a 404 error', async () => {
    const fake = fakeFetch(() => jsonResponse(404, { error: 'not_found', message: 'no such resource' }))
    await expect(createAccountsApi({ fetch: fake.fetch }).removeGame('g')).rejects.toMatchObject({
      code: 'unknown',
      status: 404,
    })
  })

  it('changes the password with a POST of both passwords as JSON, and accepts the empty 204', async () => {
    const fake = fakeFetch(() => new Response(null, { status: 204 }))
    const change = { current_password: 'the old passphrase', new_password: 'the new passphrase' }
    await expect(createAccountsApi({ fetch: fake.fetch }).changePassword(change)).resolves.toBeUndefined()
    expect(fake.calls[0]?.url).toBe('/api/auth/password')
    expect(fake.calls[0]?.init?.method).toBe('POST')
    expect(fake.calls[0]?.init?.credentials).toBe('include')
    expect(fake.calls[0]?.init?.headers).toEqual({ 'content-type': 'application/json' })
    expect(JSON.parse(String(fake.calls[0]?.init?.body))).toEqual(change)
  })

  it('deletes the account with a DELETE that carries the password as JSON, and accepts the empty 204', async () => {
    const fake = fakeFetch(() => new Response(null, { status: 204 }))
    const deletion = { password: 'the old passphrase' }
    await expect(createAccountsApi({ fetch: fake.fetch }).deleteAccount(deletion)).resolves.toBeUndefined()
    expect(fake.calls[0]?.url).toBe('/api/auth/account')
    expect(fake.calls[0]?.init?.method).toBe('DELETE')
    expect(fake.calls[0]?.init?.credentials).toBe('include')
    expect(fake.calls[0]?.init?.headers).toEqual({ 'content-type': 'application/json' })
    expect(JSON.parse(String(fake.calls[0]?.init?.body))).toEqual(deletion)
  })

  it.each([
    [
      'changePassword',
      (api: AccountsApi) => api.changePassword({ current_password: 'a', new_password: 'b' }),
    ],
    ['deleteAccount', (api: AccountsApi) => api.deleteAccount({ password: 'a' })],
  ] as const)(
    '%s turns a refusal into an AccountError with the server’s code and wait',
    async (_name, run) => {
      const fake = fakeFetch(() => authError(429, 'too_many_attempts', 'slow down', { 'retry-after': '90' }))
      const failure: unknown = await run(createAccountsApi({ fetch: fake.fetch })).catch(
        (error: unknown) => error,
      )
      expect(failure).toBeInstanceOf(AccountError)
      expect(failure).toMatchObject({ code: 'too_many_attempts', status: 429, retryAfter: 90 })
    },
  )

  it('prefixes every path with the base url', async () => {
    const fake = fakeFetch(() => jsonResponse(200, user))
    await createAccountsApi({ fetch: fake.fetch, baseUrl: 'https://play.example' }).me()
    expect(fake.calls[0]?.url).toBe('https://play.example/api/auth/me')
  })
})

describe('answers', () => {
  it('returns the account as the server sent it', async () => {
    const fake = fakeFetch(() => jsonResponse(200, user))
    await expect(createAccountsApi({ fetch: fake.fetch }).me()).resolves.toEqual(user)
  })

  it('returns the games with their seats, usernames and result', async () => {
    const fake = fakeFetch(() => jsonResponse(200, { games: [listed(), finished] }))
    const games = await createAccountsApi({ fetch: fake.fetch }).myGames()
    expect(games).toHaveLength(2)
    expect(games[1]).toEqual(finished)
    expect(games[0]?.result).toBeNull()
  })

  it('returns a failed game with its reason, and no other game with one', async () => {
    const reason = 'The deck of Bo has changed since this game was made, so it cannot continue.'
    const failed = listed({ game: 'broken', status: 'failed', reason })
    const fake = fakeFetch(() => jsonResponse(200, { games: [failed, listed()] }))
    const games = await createAccountsApi({ fetch: fake.fetch }).myGames()
    expect(games[0]).toEqual(failed)
    expect(games[0]?.reason).toBe(reason)
    expect(games[1]?.reason).toBeNull()
  })

  it('accepts an empty list', async () => {
    const fake = fakeFetch(() => jsonResponse(200, { games: [] }))
    await expect(createAccountsApi({ fetch: fake.fetch }).myGames()).resolves.toEqual([])
  })

  const unreadableAccounts: [string, unknown][] = [
    ['an account without a username', { id: 'u-1' }],
    ['an account whose id is a number', { id: 1, username: 'ada_l' }],
    ['an array for an account', []],
    ['text for an account', 'ada_l'],
  ]

  it.each(unreadableAccounts)('rejects %s as bad_response', async (_what, body) => {
    const fake = fakeFetch(() => jsonResponse(200, body))
    await expect(createAccountsApi({ fetch: fake.fetch }).me()).rejects.toMatchObject({
      code: 'bad_response',
      status: 200,
    })
  })

  const unreadableGames: [string, unknown][] = [
    ['games that are not a list', { games: 'none' }],
    ['no games key', {}],
    ['a game that is not an object', { games: [3] }],
    ['a game with an unknown status', { games: [{ ...listed(), status: 'paused' }] }],
    ['a game without a seat number', { games: [{ ...listed(), seat: 'one' }] }],
    ['a game with a negative seat', { games: [{ ...listed(), seat: -1 }] }],
    ['a game whose others are not a list', { games: [{ ...listed(), others: null }] }],
    ['another seat without a user field', { games: [{ ...listed(), others: [{ seat: 1, name: 'Bo' }] }] }],
    [
      'another seat whose user is a number',
      { games: [{ ...listed(), others: [{ seat: 1, name: 'Bo', user: 3 }] }] },
    ],
    ['a result that is neither Draw nor a Winner', { games: [{ ...listed(), result: 'Won' }] }],
    ['a game without a result field', { games: [{ ...listed(), result: undefined }] }],
    ['a game without a reason field', { games: [{ ...listed(), reason: undefined }] }],
    ['a game whose reason is a number', { games: [{ ...listed(), reason: 3 }] }],
  ]

  it.each(unreadableGames)('rejects %s as bad_response', async (_what, body) => {
    const fake = fakeFetch(() => jsonResponse(200, body))
    await expect(createAccountsApi({ fetch: fake.fetch }).myGames()).rejects.toMatchObject({
      code: 'bad_response',
      status: 200,
    })
  })

  it('rejects a body that is not JSON as bad_response', async () => {
    const fake = fakeFetch(() => new Response('<html>', { status: 200 }))
    await expect(createAccountsApi({ fetch: fake.fetch }).me()).rejects.toMatchObject({
      code: 'bad_response',
    })
  })
})

describe('failures', () => {
  // Every code the server sends, with the status it sends it with (docs/server.md, Accounts).
  const statusOf: Record<string, number> = {
    invalid_request: 400,
    invalid_username: 400,
    invalid_password: 400,
    username_taken: 409,
    registration_closed: 403,
    invalid_credentials: 401,
    too_many_attempts: 429,
    unauthenticated: 401,
    forbidden_origin: 403,
    payload_too_large: 413,
    unsupported_media_type: 415,
    accounts_unavailable: 503,
    unavailable: 503,
    internal: 500,
  }

  it('knows exactly the codes of the generated union', () => {
    expect([...AUTH_ERROR_CODES].sort()).toEqual(Object.keys(statusOf).sort())
    expect(AUTH_ERROR_CODES.every((code) => isAuthErrorCode(code))).toBe(true)
    expect(isAuthErrorCode('not_found')).toBe(false)
  })

  it.each(AUTH_ERROR_CODES.map((code) => [code, statusOf[code] ?? 0] as const))(
    'turns %s (HTTP %i) into an AccountError with that code',
    async (code, status) => {
      const fake = fakeFetch(() => authError(status, code, 'the server’s words'))
      const error = await createAccountsApi({ fetch: fake.fetch })
        .login(credentials)
        .catch((e: unknown) => e)
      expect(error).toBeInstanceOf(AccountError)
      expect(error).toMatchObject({ code, status, message: 'the server’s words', retryAfter: null })
    },
  )

  it.each([
    ['900', 900],
    [' 30 ', 30],
    ['Wed, 21 Oct 2026 07:28:00 GMT', null],
    ['-5', null],
    ['soon', null],
  ])('reads Retry-After %j as %j seconds', async (header, seconds) => {
    const fake = fakeFetch(() => authError(429, 'too_many_attempts', 'slow down', { 'retry-after': header }))
    await expect(createAccountsApi({ fetch: fake.fetch }).login(credentials)).rejects.toMatchObject({
      code: 'too_many_attempts',
      retryAfter: seconds,
    })
  })

  it('keeps a code it does not know as unknown, with the server’s message', async () => {
    const fake = fakeFetch(() => jsonResponse(418, { error: 'teapot', message: 'short and stout' }))
    await expect(createAccountsApi({ fetch: fake.fetch }).me()).rejects.toMatchObject({
      code: 'unknown',
      status: 418,
      message: 'short and stout',
    })
  })

  it.each([
    ['an HTML page', new Response('<html>bad gateway</html>', { status: 502 })],
    ['an empty body', new Response(null, { status: 500 })],
    ['JSON without the error fields', jsonResponse(500, { oops: true })],
  ])('turns %s into bad_response with the status', async (_what, response) => {
    const fake = fakeFetch(() => response)
    await expect(createAccountsApi({ fetch: fake.fetch }).me()).rejects.toMatchObject({
      code: 'bad_response',
      status: response.status,
    })
  })

  it('turns a failed request into network, status 0', async () => {
    const api = createAccountsApi({
      fetch: () => Promise.reject(new TypeError('Failed to fetch')),
    })
    await expect(api.me()).rejects.toMatchObject({ code: 'network', status: 0 })
    await expect(api.logout()).rejects.toMatchObject({ code: 'network', status: 0 })
  })
})
