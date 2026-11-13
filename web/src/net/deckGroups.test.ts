// Goal: the deck-group client sends each call to its endpoint and method with the session cookie and a JSON body, reads only well-formed groups, treats an answer without content as done, and turns every failure into a DeckError.
import { describe, expect, it } from 'vitest'
import { fakeFetch, jsonResponse } from '@/test-utils/fakeFetch'
import type { FetchCall } from '@/test-utils/fakeFetch'
import { createDeckGroupsApi } from './deckGroups'
import { DeckError } from './decks'

const FUN = { id: 'g1', name: 'Fun', collapsed: false, decks: ['d1'] }

function serve(answer: (call: FetchCall) => Response | Promise<Response>, baseUrl?: string) {
  const fake = fakeFetch(answer)
  return { api: createDeckGroupsApi({ fetch: fake.fetch, baseUrl }), calls: fake.calls }
}

describe('createDeckGroupsApi', () => {
  it('lists the groups with the session cookie', async () => {
    const { api, calls } = serve(() => jsonResponse(200, { groups: [FUN] }))
    expect(await api.list()).toEqual([FUN])
    expect(calls[0]?.url).toBe('/api/deck-groups')
    expect(calls[0]?.init?.credentials).toBe('include')
  })

  it.each([
    [
      'create',
      (api: ReturnType<typeof createDeckGroupsApi>) => api.create('Fun'),
      '/api/deck-groups',
      'POST',
      { name: 'Fun' },
    ],
    [
      'change',
      (api: ReturnType<typeof createDeckGroupsApi>) => api.change('g1', { collapsed: true }),
      '/api/deck-groups/g1',
      'PATCH',
      { collapsed: true },
    ],
    [
      'reorder',
      (api: ReturnType<typeof createDeckGroupsApi>) => api.reorder(['g2', 'g1']),
      '/api/deck-groups/order',
      'PUT',
      { ids: ['g2', 'g1'] },
    ],
    [
      'file',
      (api: ReturnType<typeof createDeckGroupsApi>) => api.file('d1', 'g1'),
      '/api/decks/d1/group',
      'PUT',
      { group: 'g1' },
    ],
    [
      'unfile',
      (api: ReturnType<typeof createDeckGroupsApi>) => api.file('d1', null),
      '/api/decks/d1/group',
      'PUT',
      { group: null },
    ],
  ] as const)('%s sends its body to its endpoint', async (_, run, url, method, body) => {
    const { api, calls } = serve(
      ({ init }) => (init?.method === 'PUT' ? new Response(null, { status: 204 }) : jsonResponse(200, FUN)),
      'https://play.example',
    )
    await run(api)
    expect(calls[0]?.url).toBe(`https://play.example${url}`)
    expect(calls[0]?.init?.method).toBe(method)
    expect(JSON.parse(String(calls[0]?.init?.body))).toEqual(body)
  })

  it('deletes with a DELETE on the group', async () => {
    const { api, calls } = serve(() => new Response(null, { status: 204 }))
    await api.remove('g1')
    expect([calls[0]?.url, calls[0]?.init?.method]).toEqual(['/api/deck-groups/g1', 'DELETE'])
  })

  it.each([
    ['no groups', {}],
    ['a group without decks', { groups: [{ id: 'g', name: 'x', collapsed: false }] }],
    ['a collapsed that is no flag', { groups: [{ ...FUN, collapsed: 'yes' }] }],
  ])('refuses a list with %s', async (_, body) => {
    const { api } = serve(() => jsonResponse(200, body))
    await expect(api.list()).rejects.toMatchObject({ code: 'bad_response' })
  })

  it.each([
    ['group_limit', 409],
    ['not_found', 404],
    ['unauthenticated', 401],
  ] as const)('rejects %s with a DeckError carrying code and status', async (code, status) => {
    const { api } = serve(() => jsonResponse(status, { error: code, message: code }))
    const error = await api.create('Fun').catch((caught: unknown) => caught)
    expect(error).toBeInstanceOf(DeckError)
    expect(error).toMatchObject({ code, status })
  })

  it('says network when no answer arrives', async () => {
    const { api } = serve(() => Promise.reject(new TypeError('offline')))
    await expect(api.list()).rejects.toMatchObject({ code: 'network', status: 0 })
  })
})
