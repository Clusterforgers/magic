// Goal: the read client sends each call to its endpoint with the session cookie and the right body, returns only answers that match the generated types (the read sheet included), and turns every failure into a DeckError whose code is the server's (stale_read among them) or a local network or bad_response.
import { describe, expect, it } from 'vitest'
import { READ_CARDS, READ_QUEUE } from '@/features/reads/dev/fixture'
import { fakeFetch, jsonResponse } from '@/test-utils/fakeFetch'
import type { FetchCall } from '@/test-utils/fakeFetch'
import { DeckError } from './decks'
import { createReadsApi } from './reads'

const CARD = Object.values(READ_CARDS)[0]!
const ORACLE = CARD.oracle_id

function serve(answer: (call: FetchCall) => Response | Promise<Response>, baseUrl?: string) {
  const fake = fakeFetch(answer)
  return { api: createReadsApi({ fetch: fake.fetch, baseUrl }), calls: fake.calls }
}

describe('createReadsApi', () => {
  it.each([
    [{ deck: 'a b' }, '/api/reads?deck=a%20b'],
    [{ precon: 'c18-adaptive' }, '/api/reads?precon=c18-adaptive'],
    [{ mine: true as const }, '/api/reads?mine=true'],
  ])('asks for the list of %j with the session cookie', async (target, url) => {
    const { api, calls } = serve(() => jsonResponse(200, READ_QUEUE))
    expect(await api.queue(target)).toEqual(READ_QUEUE)
    expect(calls[0]?.url).toBe(url)
    expect(calls[0]?.init?.credentials).toBe('include')
  })

  it('reads a card with its sheet, and sends a verdict as a PUT of the hash, the verdict and the note', async () => {
    const { api, calls } = serve(() => jsonResponse(200, CARD), 'https://play.example')
    expect(await api.get(ORACLE)).toEqual(CARD)
    await api.judge(ORACLE, { read_hash: CARD.read_hash, verdict: 'wrong', note: 'draws two' })
    expect(calls.map((call) => [call.url, call.init?.method ?? 'GET', call.init?.body])).toEqual([
      [`https://play.example/api/reads/${ORACLE}`, 'GET', undefined],
      [
        `https://play.example/api/reads/${ORACLE}`,
        'PUT',
        JSON.stringify({ read_hash: CARD.read_hash, verdict: 'wrong', note: 'draws two' }),
      ],
    ])
  })

  it.each([
    ['a state that is no word', { ...CARD, state: 'maybe' }],
    [
      'a row without steps',
      { ...CARD, sheet: { ...CARD.sheet, rows: [{ ...CARD.sheet.rows[0], steps: 'x' }] } },
    ],
    [
      'a row state of another shape',
      { ...CARD, sheet: { ...CARD.sheet, rows: [{ ...CARD.sheet.rows[0], state: { odd: {} } }] } },
    ],
    ['an own verdict without a time', { ...CARD, mine: { verdict: 'correct', note: null, current: true } }],
    ['no sheet', { ...CARD, sheet: null }],
  ])('refuses %s as bad_response', async (_, body) => {
    const { api } = serve(() => jsonResponse(200, body))
    await expect(api.get(ORACLE)).rejects.toMatchObject({ code: 'bad_response' })
  })

  it.each([
    [409, 'stale_read'],
    [404, 'not_found'],
    [429, 'rate_limited'],
    [400, 'invalid_request'],
    [418, 'unknown'],
  ])('turns a %i %s into that DeckError', async (status, code) => {
    const error = code === 'unknown' ? 'teapot' : code
    const { api } = serve(() => jsonResponse(status, { error, message: 'said' }))
    const failed = await api.judge(ORACLE, { read_hash: 'x', verdict: 'correct', note: null }).catch((e) => e)
    expect(failed).toBeInstanceOf(DeckError)
    expect(failed).toMatchObject({ code, status })
  })

  it('names a network failure', async () => {
    const { api } = serve(() => Promise.reject(new TypeError('offline')))
    await expect(api.queue({ deck: 'd' })).rejects.toMatchObject({ code: 'network', status: 0 })
  })
})
