// Goal: the host's close sends a DELETE for the game with the seat token and the session, resolves on the documented 204 only, refuses a token that cannot go in a header before sending anything, and fails with a LobbyError whose code is the server's.
import { describe, expect, it } from 'vitest'
import { fakeFetch, jsonResponse } from '@/test-utils/fakeFetch'
import { createApi } from './api'
import { LobbyError, describeLobbyError } from './lobbyErrors'

const GAME = '6f1e0c1a-3b1f-4d9e-8a3e-0d3c2d1b7a55'
const TOKEN = 'a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90'

const empty = (status: number) => new Response(null, { status })

async function failureOf(promise: Promise<unknown>): Promise<LobbyError> {
  try {
    await promise
  } catch (error) {
    if (error instanceof LobbyError) return error
    throw error
  }
  throw new Error('the call succeeded but should have failed')
}

describe('closeGame', () => {
  it('sends DELETE /api/games/{id} with the token and the session, and resolves on 204', async () => {
    const fake = fakeFetch(() => empty(204))
    await expect(createApi({ fetch: fake.fetch }).closeGame(GAME, { token: TOKEN })).resolves.toBeUndefined()
    expect(fake.calls).toHaveLength(1)
    expect(fake.calls[0]?.url).toBe(`/api/games/${GAME}`)
    expect(fake.calls[0]?.init).toEqual({
      method: 'DELETE',
      credentials: 'include',
      headers: { authorization: `Bearer ${TOKEN}` },
    })
  })

  it('leaves the token out when the signed-in session is the credential', async () => {
    const fake = fakeFetch(() => empty(204))
    await createApi({ fetch: fake.fetch }).closeGame(GAME)
    expect(fake.calls[0]?.init?.headers).toEqual({})
  })

  it('escapes the game id in the path', async () => {
    const fake = fakeFetch(() => empty(204))
    await createApi({ fetch: fake.fetch }).closeGame('a/b?c')
    expect(fake.calls[0]?.url).toBe('/api/games/a%2Fb%3Fc')
  })

  it('sends nothing for a token that cannot be a header', async () => {
    const fake = fakeFetch(() => empty(204))
    const error = await failureOf(createApi({ fetch: fake.fetch }).closeGame(GAME, { token: 'bad token\n' }))
    expect([error.code, error.status]).toEqual(['invalid_request', 0])
    expect(fake.calls).toHaveLength(0)
  })

  it.each([
    [403, 'forbidden', /only the host/i],
    [404, 'not_found', /no open game matches/i],
    [409, 'started', /already started/i],
    [503, 'unavailable', /busy or restarting/i],
    [401, 'unauthenticated', /not signed in/i],
  ])(
    'fails with the server’s code on HTTP %i %s and words a player can read',
    async (status, code, words) => {
      const fake = fakeFetch(() => jsonResponse(status, { error: code, message: 'raw words' }))
      const error = await failureOf(createApi({ fetch: fake.fetch }).closeGame(GAME, { token: TOKEN }))
      expect([error.code, error.status]).toEqual([code, status])
      expect(describeLobbyError(error)).toMatch(words)
    },
  )

  it.each([
    ['a 200 with a body', () => jsonResponse(200, {})],
    ['a 200 with nothing', () => empty(200)],
  ])('does not take %s for a close', async (_what, answer) => {
    const fake = fakeFetch(answer)
    const error = await failureOf(createApi({ fetch: fake.fetch }).closeGame(GAME, { token: TOKEN }))
    expect(error.code).toBe('bad_response')
  })

  it('says the server cannot be reached when the request never gets an answer', async () => {
    const fake = fakeFetch(() => Promise.reject(new TypeError('Failed to fetch')))
    const error = await failureOf(createApi({ fetch: fake.fetch }).closeGame(GAME, { token: TOKEN }))
    expect([error.code, error.status]).toEqual(['network', 0])
  })
})
