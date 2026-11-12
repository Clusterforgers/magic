// Goal: the art client calls each endpoint with the documented method, path and body, reads a printing an older server sends without its look fields, and turns refusals and unreadable answers into deck errors.
import { describe, expect, it } from 'vitest'
import { DeckError, readPictures } from './decks'
import { createPrintingsApi } from './printings'

interface Call {
  url: string
  method: string
  body: unknown
}

function serve(status: number, body: unknown) {
  const calls: Call[] = []
  const fetch = (url: string, init?: RequestInit) => {
    calls.push({
      url,
      method: init?.method ?? 'GET',
      body: typeof init?.body === 'string' ? JSON.parse(init.body) : undefined,
    })
    const text = body === undefined ? null : JSON.stringify(body)
    return Promise.resolve(new Response(text, { status }))
  }
  return { calls, api: createPrintingsApi({ fetch, baseUrl: '' }) }
}

const OLD_PRINTING = {
  printing_id: 'p1',
  set: 'lea',
  collector_number: '161',
  rarity: 'common',
  released_at: '1993-08-05',
  lang: 'en',
  digital: false,
  images: { card: null, faces: [] },
}

describe('the art client', () => {
  it('lists printings and fills the look an older server leaves out', async () => {
    const { calls, api } = serve(200, { printings: [OLD_PRINTING] })
    const printings = await api.printingsOf('o/1')
    expect(calls[0]).toMatchObject({ url: '/api/cards/o%2F1/printings', method: 'GET' })
    expect(printings).toEqual([{ ...OLD_PRINTING, set_name: null, frame: null, finishes: [], full_art: false }])
  })

  it.each([
    ['a line', (api: ReturnType<typeof createPrintingsApi>) => api.setLine('d1', 'commander', 'o1', 'p1'), 'PUT', '/api/decks/d1/printing', { zone: 'commander', oracle_id: 'o1', printing_id: 'p1' }],
    ['a line back to default', (api: ReturnType<typeof createPrintingsApi>) => api.setLine('d1', 'main', 'o1', null), 'PUT', '/api/decks/d1/printing', { zone: 'main', oracle_id: 'o1', printing_id: null }],
    ['the preferred art', (api: ReturnType<typeof createPrintingsApi>) => api.prefer('o1', 'p1'), 'PUT', '/api/me/printings/o1', { printing_id: 'p1' }],
    ['no preferred art', (api: ReturnType<typeof createPrintingsApi>) => api.prefer('o1', null), 'DELETE', '/api/me/printings/o1', undefined],
  ])('sets %s', async (_what, act, method, url, body) => {
    const { calls, api } = serve(204, undefined)
    await act(api)
    expect(calls).toEqual([{ url, method, body }])
  })

  it('reads the preferred art', async () => {
    const { api } = serve(200, { printings: { o1: 'p1' } })
    expect(await api.preferred()).toEqual({ o1: 'p1' })
  })

  it.each([
    [400, { error: 'invalid_deck', message: 'Not a printing of the card.' }, 'invalid_deck'],
    [404, { error: 'not_found', message: 'no such resource' }, 'not_found'],
    [500, 'oops', 'bad_response'],
  ])('turns HTTP %i into a %s error', async (status, body, code) => {
    const { api } = serve(status, body)
    await expect(api.setLine('d1', 'main', 'o1', 'p1')).rejects.toMatchObject({ code })
    await expect(api.setLine('d1', 'main', 'o1', 'p1')).rejects.toBeInstanceOf(DeckError)
  })

  it('refuses a list it cannot read', async () => {
    const { api } = serve(200, { printings: [{ printing_id: 1 }] })
    await expect(api.printingsOf('o1')).rejects.toMatchObject({ code: 'bad_response' })
  })
})

describe('the pictures of a deck in a list', () => {
  it.each([
    ['a chosen printing', [{ oracle_id: 'o', name: 'N', printing_id: 'p' }], [{ oracle_id: 'o', name: 'N', printing_id: 'p' }]],
    ['no printing', [{ oracle_id: 'o', name: 'N' }], [{ oracle_id: 'o', name: 'N' }]],
    ['a printing that is no text', [{ oracle_id: 'o', name: 'N', printing_id: 3 }], null],
  ])('reads %s', (_what, given, read) => {
    expect(readPictures(given)).toEqual(read)
  })
})
