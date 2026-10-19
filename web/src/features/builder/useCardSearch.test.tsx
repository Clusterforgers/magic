// Goal: the search waits for typing to pause and sends one request for the last thing typed, cancels the one before it, starts again at page 1 when a filter changes, keeps the rows it has while the next page loads, makes no request with nothing to search for, and tells a failure, a server without cards and a retry apart.
import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { CardsError } from '@/net/cards'
import type { CardQuery, CardsApi } from '@/net/cards'
import type { CardSearchPage } from '@/protocol/generated'
import { cardHit, searchPage } from './fakeCards'
import { NO_FILTERS } from './search'
import type { SearchFilters } from './search'
import { SEARCH_PAUSE_MS, useCardSearch } from './useCardSearch'

beforeEach(() => {
  vi.useFakeTimers()
})
afterEach(() => {
  vi.useRealTimers()
})

interface Pending {
  query: CardQuery
  signal: AbortSignal | undefined
  resolve: (page: CardSearchPage) => void
  reject: (error: unknown) => void
}

/** A cards client whose answers the test gives by hand, so it decides what is in flight. */
function manual() {
  const asked: Pending[] = []
  const api: CardsApi = {
    search: (query, signal) =>
      new Promise<CardSearchPage>((resolve, reject) => {
        asked.push({ query, signal, resolve, reject })
      }),
    get: () => Promise.reject(new Error('not used')),
  }
  return { api, asked }
}

const withText = (text: string): SearchFilters => ({ ...NO_FILTERS, text })
const settle = (ms = SEARCH_PAUSE_MS) => act(() => vi.advanceTimersByTimeAsync(ms))

function setup(initial: SearchFilters = NO_FILTERS) {
  const api = manual()
  const hook = renderHook(({ filters }) => useCardSearch(api.api, filters), {
    initialProps: { filters: initial },
  })
  return { ...api, ...hook }
}

describe('with nothing to search for', () => {
  it('is idle and asks nothing, however long it waits', async () => {
    const { result, asked } = setup()
    await settle(5_000)
    expect(result.current.state).toEqual({ status: 'idle' })
    expect(asked).toEqual([])
  })

  it('goes back to idle when the filters are cleared', async () => {
    const { result, rerender, asked } = setup(withText('bolt'))
    await settle(0)
    await act(async () => asked[0]?.resolve(searchPage([cardHit('Lightning Bolt')])))
    expect(result.current.state.status).toBe('ready')
    rerender({ filters: NO_FILTERS })
    await settle()
    expect(result.current.state).toEqual({ status: 'idle' })
  })
})

describe('the pause', () => {
  it('sends one request, for the last text, once typing has paused', async () => {
    const { result, rerender, asked } = setup()
    rerender({ filters: withText('b') })
    await settle(100)
    rerender({ filters: withText('bo') })
    await settle(100)
    rerender({ filters: withText('bol') })
    expect(result.current.pending).toBe(true)
    await settle(SEARCH_PAUSE_MS - 1)
    expect(asked).toEqual([])
    await settle(1)
    expect(asked.map((item) => item.query.text)).toEqual(['bol'])
    expect(result.current.pending).toBe(false)
  })

  it('searches at once when asked to flush', async () => {
    const { result, rerender, asked } = setup()
    rerender({ filters: withText('bolt') })
    expect(asked).toEqual([])
    act(() => result.current.flush())
    await settle(0)
    expect(asked.map((item) => item.query.text)).toEqual(['bolt'])
  })

  it('does not ask again for filters that come back equal', async () => {
    const { rerender, asked } = setup(withText('bolt'))
    await settle(0)
    rerender({ filters: { ...withText('bolt') } })
    await settle()
    expect(asked).toHaveLength(1)
  })

  it('cancels the request before it when a newer search starts', async () => {
    const { rerender, asked } = setup(withText('a'))
    await settle(0)
    rerender({ filters: withText('ab') })
    await settle()
    expect(asked.map((item) => item.signal?.aborted)).toEqual([true, false])
  })

  it('ignores the answer of a search that was replaced', async () => {
    const { result, rerender, asked } = setup(withText('a'))
    await settle(0)
    rerender({ filters: withText('ab') })
    await settle()
    await act(async () => asked[0]?.resolve(searchPage([cardHit('Old')])))
    expect(result.current.state.status).toBe('loading')
    await act(async () => asked[1]?.resolve(searchPage([cardHit('New')])))
    expect(result.current.state).toMatchObject({ status: 'ready', page: { cards: [{ name: 'New' }] } })
  })
})

describe('paging', () => {
  const page = (number: number) =>
    searchPage([cardHit(`Card ${number}`)], { page: number, total: 90, per_page: 40 })

  it('asks for the page it is sent to and keeps the rows it has while that loads', async () => {
    const { result, asked } = setup(withText('card'))
    await settle(0)
    await act(async () => asked[0]?.resolve(page(1)))
    act(() => result.current.goTo(2))
    await settle(0)
    expect(asked[1]?.query).toMatchObject({ text: 'card', page: 2 })
    expect(result.current.page).toBe(2)
    expect(result.current.state).toMatchObject({ status: 'loading', previous: { page: 1 } })
    await act(async () => asked[1]?.resolve(page(2)))
    expect(result.current.state).toMatchObject({ status: 'ready', page: { page: 2 } })
  })

  it('never goes below page 1', async () => {
    const { result, asked } = setup(withText('card'))
    await settle(0)
    act(() => result.current.goTo(-4))
    expect(result.current.page).toBe(1)
    expect(asked).toHaveLength(1)
  })

  it('starts again at page 1 when a filter changes', async () => {
    const { result, rerender, asked } = setup(withText('card'))
    await settle(0)
    await act(async () => asked[0]?.resolve(page(1)))
    act(() => result.current.goTo(3))
    await settle(0)
    expect(result.current.page).toBe(3)
    rerender({ filters: { ...withText('card'), type: 'creature' } })
    await settle()
    expect(result.current.page).toBe(1)
    expect(asked.at(-1)?.query).toMatchObject({ type: 'creature', page: 1 })
  })
})

describe('failures', () => {
  it('words a failed search and asks again on retry', async () => {
    const { result, asked } = setup(withText('bolt'))
    await settle(0)
    await act(async () => asked[0]?.reject(new CardsError('network', 0, 'x')))
    expect(result.current.state).toMatchObject({
      status: 'failed',
      message: expect.stringMatching(/could not reach/i),
    })
    act(() => result.current.retry())
    await settle(0)
    expect(asked).toHaveLength(2)
    expect(result.current.state.status).toBe('loading')
    await act(async () => asked[1]?.resolve(searchPage([])))
    expect(result.current.state).toMatchObject({ status: 'ready' })
  })

  it('tells a server without a card database from a failure', async () => {
    const { result, asked } = setup(withText('bolt'))
    await settle(0)
    await act(async () => asked[0]?.reject(new CardsError('cards_unavailable', 503, 'no database')))
    expect(result.current.state).toEqual({ status: 'unavailable' })
  })

  it('words an error that is not a CardsError too', async () => {
    const { result, asked } = setup(withText('bolt'))
    await settle(0)
    await act(async () => asked[0]?.reject(new Error('boom')))
    expect(result.current.state).toMatchObject({
      status: 'failed',
      message: expect.stringMatching(/unexpected/i),
    })
  })
})
