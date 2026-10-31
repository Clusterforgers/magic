// Goal: the favourites client sends each call to its endpoint with the session cookie, reads only a list of codes, treats an answer without content as done, and turns every failure into a DeckError whose code is the server's or a local network or bad_response.
import { describe, expect, it } from 'vitest'
import { fakeFetch, jsonResponse } from '@/test-utils/fakeFetch'
import type { FetchCall } from '@/test-utils/fakeFetch'
import { DeckError } from './decks'
import { createFavouritesApi } from './favourites'

function serve(answer: (call: FetchCall) => Response | Promise<Response>, baseUrl?: string) {
  const fake = fakeFetch(answer)
  return { api: createFavouritesApi({ fetch: fake.fetch, baseUrl }), calls: fake.calls }
}

const refusal = (status: number, error: string) =>
  jsonResponse(status, { error, message: `server said ${error}` })

describe('createFavouritesApi', () => {
  it('lists the codes with the session cookie', async () => {
    const { api, calls } = serve(() => jsonResponse(200, { codes: ['c19-merciless-rage', 'c18-x'] }))
    expect(await api.list()).toEqual(['c19-merciless-rage', 'c18-x'])
    expect(calls[0]?.url).toBe('/api/favourites/precons')
    expect(calls[0]?.init?.credentials).toBe('include')
  })

  it('stars with a PUT and unstars with a DELETE on the code, under the base URL', async () => {
    const { api, calls } = serve(() => new Response(null, { status: 204 }), 'https://play.example')
    await api.star('c18-draconic-domination')
    await api.unstar('c18-draconic-domination')
    expect(calls.map((call) => [call.url, call.init?.method])).toEqual([
      ['https://play.example/api/favourites/precons/c18-draconic-domination', 'PUT'],
      ['https://play.example/api/favourites/precons/c18-draconic-domination', 'DELETE'],
    ])
    expect(calls[0]?.init?.credentials).toBe('include')
  })

  it.each([
    ['not an object', 'x'],
    ['no codes', {}],
    ['codes of another kind', { codes: [1] }],
    ['codes that are no list', { codes: 'c18' }],
  ])('refuses a list that is %s', async (_, body) => {
    const { api } = serve(() => jsonResponse(200, body))
    await expect(api.list()).rejects.toMatchObject({ code: 'bad_response' })
  })

  it.each([
    ['unauthenticated', 401],
    ['not_found', 404],
    ['rate_limited', 429],
    ['cards_unavailable', 503],
  ] as const)('rejects %s with a DeckError that carries the code and the status', async (code, status) => {
    const { api } = serve(() => refusal(status, code))
    const error = await api.star('c18-x').catch((caught: unknown) => caught)
    expect(error).toBeInstanceOf(DeckError)
    expect(error).toMatchObject({ code, status })
  })

  it('says bad_response for a failure with no readable error and network for no answer', async () => {
    const odd = serve(() => new Response('oops', { status: 500 }))
    await expect(odd.api.unstar('c18-x')).rejects.toMatchObject({ code: 'bad_response', status: 500 })
    const down = serve(() => Promise.reject(new TypeError('offline')))
    await expect(down.api.list()).rejects.toMatchObject({ code: 'network', status: 0 })
  })
})
