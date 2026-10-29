// Goal: the precon client reads the list and one precon's details with every field checked, sends the code in the path, and turns every failure (a server error with its code, a 429 with its wait, an answer it cannot read, no answer) into a PreconError with a stable code and words.
import { describe, expect, it } from 'vitest'
import { checked, preconItem, preconView, startPrecons } from '@/features/precons/fakePrecons'
import { describePreconError } from '@/features/precons/errors'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { createPreconsApi, PRECON_ERROR_CODES, PreconError } from './precons'

const failing = (response: () => Response | Promise<Response>) => {
  const calls: string[] = []
  const api = createPreconsApi({
    fetch: async (url) => {
      calls.push(url)
      return response()
    },
  })
  return { api, calls }
}

describe('list and get', () => {
  it('reads the list in the order the server sends it and asks the right path', async () => {
    const list = [checked(preconItem({ code: 'b-deck' }), { manual: 2 }), preconItem({ code: 'a-deck' })]
    const server = startPrecons({ precons: list })
    expect(await server.api.list()).toEqual({ precons: list })
    expect(server.calls.map((call) => call.url)).toEqual(['/api/precons'])
  })

  it('reads one precon with its boards and escapes the code in the path', async () => {
    const item = preconItem({ code: 'c18-adaptive-enchantment' })
    const server = startPrecons({ precons: [item] })
    const view = await server.api.get('c18-adaptive-enchantment')
    expect(view).toEqual(preconView(item))
    expect(server.calls.map((call) => call.url)).toEqual(['/api/precons/c18-adaptive-enchantment'])
    const { api, calls } = failing(() => jsonResponse(404, { error: 'not_found', message: 'no' }))
    await expect(api.get('a/b?c')).rejects.toBeInstanceOf(PreconError)
    expect(calls).toEqual(['/api/precons/a%2Fb%3Fc'])
  })
})

describe('failures', () => {
  it.each([
    [
      'a server error with its code',
      () => jsonResponse(404, { error: 'not_found', message: 'gone' }),
      'not_found',
      404,
    ],
    [
      'no card database',
      () => jsonResponse(503, { error: 'cards_unavailable', message: 'none' }),
      'cards_unavailable',
      503,
    ],
    [
      'a code this client does not know',
      () => jsonResponse(418, { error: 'teapot', message: 'tea' }),
      'unknown',
      418,
    ],
    ['an error with no body it can read', () => new Response('<html>', { status: 502 }), 'bad_response', 502],
    ['a list that is not a list', () => jsonResponse(200, { precons: 'many' }), 'bad_response', 200],
    [
      'a precon with a missing field',
      () => jsonResponse(200, { precons: [{ ...preconItem(), playable: undefined }] }),
      'bad_response',
      200,
    ],
    [
      'a precon with a state it does not know',
      () => jsonResponse(200, { precons: [{ ...preconItem(), state: 'great' }] }),
      'bad_response',
      200,
    ],
  ])('%s', async (_, answer, code, status) => {
    const { api } = failing(answer)
    await expect(api.list()).rejects.toMatchObject({ name: 'PreconError', code, status })
  })

  it('reads the wait of a 429 and puts it in the words', async () => {
    const limited = () =>
      new Response(JSON.stringify({ error: 'rate_limited', message: 'slow' }), {
        status: 429,
        headers: { 'content-type': 'application/json', 'retry-after': '17' },
      })
    const { api } = failing(limited)
    const error = await api.list().catch((caught: unknown) => caught)
    expect(error).toMatchObject({ code: 'rate_limited', status: 429, retryAfter: 17 })
    expect(describePreconError(error)).toContain('17 seconds')
  })

  it('reports a request that never arrived as a network failure', async () => {
    const api = createPreconsApi({
      fetch: () => Promise.reject(new TypeError('offline')),
    })
    await expect(api.get('x')).rejects.toMatchObject({ code: 'network', status: 0 })
  })

  it.each(PRECON_ERROR_CODES)('has words for the server code %s', (code) => {
    expect(describePreconError(new PreconError(code, 500, 'x')).length).toBeGreaterThan(20)
  })
})
