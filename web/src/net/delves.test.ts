// Goal: the Delves client sends each call to its path with its method and body, and turns a server error or no answer into a DelveError with the server's code and words.
import { describe, expect, it } from 'vitest'
import { fakeFetch, jsonResponse } from '@/test-utils/fakeFetch'
import { delveView } from '@/features/delves/fakeDelves'
import { createDelvesApi, DelveError } from './delves'

describe('delves client', () => {
  it.each([
    ['list', (api: ReturnType<typeof createDelvesApi>) => api.list(), '/api/delves', 'GET', { delves: [] }],
    [
      'create',
      (api: ReturnType<typeof createDelvesApi>) => api.create({ heat: 1 }),
      '/api/delves',
      'POST',
      delveView(),
    ],
    [
      'join',
      (api: ReturnType<typeof createDelvesApi>) => api.join(' AB C '),
      '/api/delves/join/AB%20C',
      'POST',
      delveView(),
    ],
    [
      'get',
      (api: ReturnType<typeof createDelvesApi>) => api.get('d/1'),
      '/api/delves/d%2F1',
      'GET',
      delveView(),
    ],
    [
      'act',
      (api: ReturnType<typeof createDelvesApi>) => api.act('d1', { kind: 'start' }),
      '/api/delves/d1/act',
      'POST',
      delveView(),
    ],
    [
      'content',
      (api: ReturnType<typeof createDelvesApi>) => api.content(),
      '/api/delves/content',
      'GET',
      { cards: [], bosses: [], events: [] },
    ],
  ])('%s calls its path', async (_, call, url, method, body) => {
    const server = fakeFetch(() => jsonResponse(200, body))
    const api = createDelvesApi({ fetch: server.fetch })
    await call(api)
    expect(server.calls[0]?.url).toBe(url)
    expect(server.calls[0]?.init?.method ?? 'GET').toBe(method)
  })

  it('sends an action as its JSON body', async () => {
    const server = fakeFetch(() => jsonResponse(200, delveView()))
    await createDelvesApi({ fetch: server.fetch }).act('d1', { kind: 'vote', node: 2 })
    expect(JSON.parse(String(server.calls[0]?.init?.body))).toEqual({ kind: 'vote', node: 2 })
  })

  it('turns a refusal into its code and message', async () => {
    const server = fakeFetch(() => jsonResponse(409, { error: 'refused', message: 'no vote now' }))
    const failure = await createDelvesApi({ fetch: server.fetch })
      .act('d1', { kind: 'start' })
      .catch((e: unknown) => e)
    expect(failure).toBeInstanceOf(DelveError)
    expect(failure).toMatchObject({ code: 'refused', status: 409, message: 'no vote now' })
  })

  it('turns no answer into a network error', async () => {
    const api = createDelvesApi({ fetch: () => Promise.reject(new Error('down')) })
    await expect(api.list()).rejects.toMatchObject({ code: 'network', status: 0 })
  })
})
