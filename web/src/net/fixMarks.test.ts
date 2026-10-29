// Goal: the fix-mark client sends each call to its endpoint with the session cookie and the right body, returns only answers that match the generated types, takes a mark that is already gone as done, and turns every failure into a DeckError whose code is the server's or a local network or bad_response.
import { describe, expect, it } from 'vitest'
import { fakeFetch, jsonResponse } from '@/test-utils/fakeFetch'
import type { FetchCall } from '@/test-utils/fakeFetch'
import type { FixMarkView } from '@/protocol/generated'
import { DeckError } from './decks'
import { createFixMarksApi } from './fixMarks'

const ORACLE = '4457ed35-7c10-48c8-9776-456485fdf070'
const VIEW: FixMarkView = {
  oracle_id: ORACLE,
  name: 'Sol Ring',
  note: 'adds the wrong mana',
  created_at: '2026-10-29T12:00:00Z',
}

function serve(answer: (call: FetchCall) => Response | Promise<Response>, baseUrl?: string) {
  const fake = fakeFetch(answer)
  return { api: createFixMarksApi({ fetch: fake.fetch, baseUrl }), calls: fake.calls }
}

const refusal = (status: number, error: string) =>
  jsonResponse(status, { error, message: `server said ${error}` })

describe('createFixMarksApi', () => {
  it('lists the marks with the session cookie', async () => {
    const { api, calls } = serve(() => jsonResponse(200, { marks: [VIEW, { ...VIEW, note: null }] }))
    expect(await api.list()).toEqual([VIEW, { ...VIEW, note: null }])
    expect(calls).toHaveLength(1)
    expect(calls[0]?.url).toBe('/api/fix-marks')
    expect(calls[0]?.init?.credentials).toBe('include')
  })

  it('marks a card with a PUT whose body is the note, null for none', async () => {
    const { api, calls } = serve(() => jsonResponse(200, VIEW), 'https://play.example')
    expect(await api.mark(ORACLE, 'adds the wrong mana')).toEqual(VIEW)
    await api.mark(ORACLE)
    expect(calls.map((call) => [call.url, call.init?.method, call.init?.body])).toEqual([
      [`https://play.example/api/fix-marks/${ORACLE}`, 'PUT', '{"note":"adds the wrong mana"}'],
      [`https://play.example/api/fix-marks/${ORACLE}`, 'PUT', '{"note":null}'],
    ])
    expect(calls[0]?.init?.credentials).toBe('include')
    expect(calls[0]?.init?.headers).toEqual({ 'content-type': 'application/json' })
  })

  it('takes a mark back with a DELETE and counts a mark that is already gone as done', async () => {
    const { api, calls } = serve(() => new Response(null, { status: 204 }))
    await api.unmark(ORACLE)
    expect(calls[0]?.init?.method).toBe('DELETE')
    expect(calls[0]?.url).toBe(`/api/fix-marks/${ORACLE}`)
    const gone = serve(() => refusal(404, 'not_found'))
    await expect(gone.api.unmark(ORACLE)).resolves.toBeUndefined()
  })

  it.each([
    ['unauthenticated', 401],
    ['cards_unavailable', 503],
    ['internal', 500],
  ] as const)('rejects %s with a DeckError that carries the code and the status', async (code, status) => {
    const { api } = serve(() => refusal(status, code))
    for (const call of [() => api.list(), () => api.mark(ORACLE), () => api.unmark(ORACLE)]) {
      const error: unknown = await call().then(
        () => null,
        (e: unknown) => e,
      )
      expect(error).toBeInstanceOf(DeckError)
      expect(error).toMatchObject({ code, status })
    }
  })

  it('reports a code it does not know as unknown and an unreadable error as bad_response', async () => {
    const odd = serve(() => refusal(418, 'teapot'))
    await expect(odd.api.list()).rejects.toMatchObject({ code: 'unknown', status: 418 })
    const plain = serve(() => new Response('<html>', { status: 502 }))
    await expect(plain.api.list()).rejects.toMatchObject({ code: 'bad_response', status: 502 })
  })

  it.each([
    ['a list without marks', {}],
    ['a mark without a name', { marks: [{ ...VIEW, name: 3 }] }],
    ['a note of another kind', { marks: [{ ...VIEW, note: 4 }] }],
  ])('refuses %s', async (_name, body) => {
    const { api } = serve(() => jsonResponse(200, body))
    await expect(api.list()).rejects.toMatchObject({ code: 'bad_response' })
  })

  it('refuses a mark answered with the wrong shape', async () => {
    const { api } = serve(() => jsonResponse(200, { oracle_id: ORACLE }))
    await expect(api.mark(ORACLE)).rejects.toMatchObject({ code: 'bad_response' })
  })

  it('says so when the server cannot be reached', async () => {
    const api = createFixMarksApi({
      fetch: () => {
        throw new TypeError('offline')
      },
    })
    await expect(api.list()).rejects.toMatchObject({ code: 'network', status: 0 })
  })

  it('escapes an id that is not a plain path segment', async () => {
    const { api, calls } = serve(() => new Response(null, { status: 204 }))
    await api.unmark('a/b?c')
    expect(calls[0]?.url).toBe('/api/fix-marks/a%2Fb%3Fc')
  })
})
