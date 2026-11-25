// Goal: the Delves client sends each call to its path with its method and body, and turns a server error or no answer into a DelveError with the server's code and words. A followed run hands on each view its socket sends, reports a lost socket and reconnects with the game socket's backoff until stopped.
import { describe, expect, it } from 'vitest'
import { fakeFetch, jsonResponse } from '@/test-utils/fakeFetch'
import { FakeNetwork, FakeTimers } from '@/test-utils/fakes'
import { delveView } from '@/features/delves/fakeDelves'
import type { DelveView } from '@/protocol/generated'
import { createDelvesApi, DelveError, delveSocketUrl, followDelve } from './delves'
import { FIRST_RETRY_MS } from './socket'

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
    [
      'leaderboard',
      (api: ReturnType<typeof createDelvesApi>) => api.leaderboard(),
      '/api/delves/leaderboard',
      'GET',
      { boards: [] },
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

describe('following a run', () => {
  function follow() {
    const network = new FakeNetwork()
    const timers = new FakeTimers()
    const views: DelveView[] = []
    let lost = 0
    const stop = followDelve(
      'd/1',
      { view: (view) => views.push(view), lost: () => (lost += 1) },
      { createSocket: network.factory, timers, random: () => 0, origin: 'https://play.example' },
    )
    return { network, timers, views, stop, lost: () => lost }
  }

  it.each([
    ['https://play.example', 'wss://play.example/ws/delves/d%2F1'],
    ['http://localhost:5173', 'ws://localhost:5173/ws/delves/d%2F1'],
  ])('opens the socket of %s at %s', (origin, url) => {
    expect(delveSocketUrl('d/1', origin)).toBe(url)
  })

  it('hands on each view and skips frames that are not one', () => {
    const live = follow()
    expect(live.network.last.url).toBe('wss://play.example/ws/delves/d%2F1')
    live.network.last.open()
    live.network.last.receive(delveView())
    live.network.last.receive('not json')
    live.network.last.receive({ kind: 'other' })
    live.network.last.receiveRaw(new ArrayBuffer(2))
    expect(live.views).toHaveLength(1)
    expect(live.network.last.sent).toEqual([])
  })

  it('reports a lost socket and reconnects with a growing wait, reset by a view', () => {
    const live = follow()
    live.network.last.fail()
    expect(live.lost()).toBe(1)
    expect(live.timers.pending()).toEqual([FIRST_RETRY_MS])
    live.timers.advance(FIRST_RETRY_MS)
    expect(live.network.sockets).toHaveLength(2)
    live.network.last.serverClose(1001)
    expect(live.timers.pending()).toEqual([FIRST_RETRY_MS * 2])
    live.timers.advance(FIRST_RETRY_MS * 2)
    live.network.last.receive(delveView())
    live.network.last.fail()
    expect(live.lost()).toBe(3)
    expect(live.timers.pending()).toEqual([FIRST_RETRY_MS])
  })

  it('stops: closes the socket, cancels a retry and reports nothing more', () => {
    const live = follow()
    const first = live.network.last
    live.stop()
    expect(first.closedByClient?.code).toBe(1000)
    first.receive(delveView())
    expect(live.views).toEqual([])

    const again = follow()
    again.network.last.fail()
    again.stop()
    expect(again.timers.pending()).toEqual([])
    expect(again.lost()).toBe(1)
  })
})
