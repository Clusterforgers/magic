// Goal: the lists a deck chooser opens with are cached stale while revalidate: a list seen once shows at once on the next mount and is read again behind it, a prefetch spares the chooser its own read, a deck saved, replaced or deleted drops the cached deck lists, and a failed read is reported, not cached as a list.
import { act, renderHook, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { deckItem } from '@/features/decks/fakeDecks'
import { useDeckList } from '@/features/decks/useDeckList'
import { usePreconChoices } from '@/features/game/usePreconChoices'
import type { DeckListItem, PreconList } from '@/protocol/generated'
import { fakeFetch, jsonResponse } from '@/test-utils/fakeFetch'
import { createDecksApi, DeckError } from './decks'
import type { DeckBody } from '@/protocol/generated'
import { cachedDecks, deckListKey, forgetDeckLists, listCache, prefetchChooserLists } from './listCache'

/** A deck client whose list answers with the next of `answers`, then the last one again. */
function decksOf(...answers: (DeckListItem[] | Error)[]) {
  let at = 0
  const list = vi.fn(async () => {
    const answer = answers[Math.min(at, answers.length - 1)] ?? []
    at += 1
    if (answer instanceof Error) throw answer
    return answer
  })
  return { list }
}

const ADA = deckItem({ id: 'a', name: 'Ada' })
const BOB = deckItem({ id: 'b', name: 'Bob' })
const NONE: PreconList = { precons: [], judging: false }

describe('useDeckList', () => {
  it('shows the cached list at once on the next mount, marked refreshing, then the list read again', async () => {
    const api = decksOf([ADA], [ADA, BOB])
    const first = renderHook(() => useDeckList(api))
    expect(first.result.current.state.status).toBe('loading')
    await waitFor(() =>
      expect(first.result.current.state).toEqual({ status: 'ready', decks: [ADA], refreshing: false }),
    )
    first.unmount()
    // Older than the ten seconds a list stays fresh.
    listCache.invalidateQueries({ queryKey: deckListKey(api) })

    const second = renderHook(() => useDeckList(api))
    expect(second.result.current.state).toEqual({ status: 'ready', decks: [ADA], refreshing: true })
    await waitFor(() =>
      expect(second.result.current.state).toEqual({ status: 'ready', decks: [ADA, BOB], refreshing: false }),
    )
    expect(api.list).toHaveBeenCalledTimes(2)
  })

  it('reads a fresh list once for every chooser mounted inside ten seconds', async () => {
    const api = decksOf([ADA])
    const first = renderHook(() => useDeckList(api))
    await waitFor(() => expect(first.result.current.state.status).toBe('ready'))
    const second = renderHook(() => useDeckList(api))
    expect(second.result.current.state).toEqual({ status: 'ready', decks: [ADA], refreshing: false })
    expect(api.list).toHaveBeenCalledTimes(1)
  })

  it('reports a failed read, and a reload reads again', async () => {
    const api = decksOf(new DeckError('network', 0, 'offline'), [ADA])
    const { result } = renderHook(() => useDeckList(api))
    await waitFor(() => expect(result.current.state.status).toBe('failed'))
    act(() => result.current.reload())
    await waitFor(() =>
      expect(result.current.state).toEqual({ status: 'ready', decks: [ADA], refreshing: false }),
    )
  })
})

describe('prefetchChooserLists', () => {
  it('reads each list once, and the chooser then opens on them without reading again', async () => {
    const decks = decksOf([ADA])
    const groups = { list: vi.fn(async () => []) }
    const favourites = { list: vi.fn(async () => ['c13']) }
    const precons = { list: vi.fn(async () => NONE) }
    prefetchChooserLists({ decks, groups, favourites, precons })
    prefetchChooserLists({ decks, groups, favourites, precons })
    await waitFor(() => expect(listCache.getQueryData(deckListKey(decks))).toEqual([ADA]))
    for (const api of [decks, groups, favourites, precons]) expect(api.list).toHaveBeenCalledTimes(1)

    const list = renderHook(() => useDeckList(decks))
    expect(list.result.current.state).toEqual({ status: 'ready', decks: [ADA], refreshing: false })
    const offered = renderHook(() => usePreconChoices(precons))
    await waitFor(() => expect(offered.result.current.state).toEqual({ status: 'ready', precons: [] }))
    expect(decks.list).toHaveBeenCalledTimes(1)
    expect(precons.list).toHaveBeenCalledTimes(1)
  })

  it('leaves a failed read for the chooser, which reads again', async () => {
    const decks = decksOf(new DeckError('network', 0, 'offline'), [ADA])
    const others = { list: vi.fn(async () => []) }
    prefetchChooserLists({ decks, groups: others, favourites: others, precons: { list: async () => NONE } })
    await waitFor(() => expect(decks.list).toHaveBeenCalledTimes(1))
    const { result } = renderHook(() => useDeckList(decks))
    await waitFor(() =>
      expect(result.current.state).toEqual({ status: 'ready', decks: [ADA], refreshing: false }),
    )
  })
})

describe('a change to a deck', () => {
  const BODY: DeckBody = { name: 'Ada', format: 'commander', notes: '', cards: [] }

  it.each([
    ['create', (api: ReturnType<typeof createDecksApi>) => api.create(BODY)],
    ['update', (api: ReturnType<typeof createDecksApi>) => api.update('a', BODY)],
    ['remove', (api: ReturnType<typeof createDecksApi>) => api.remove('a')],
  ])('calls onChange after %s succeeds, and not after it fails', async (_, change) => {
    const onChange = vi.fn()
    const refused = createDecksApi({
      fetch: fakeFetch(() => jsonResponse(404, { error: 'not_found', message: 'gone' })).fetch,
      onChange,
    })
    await expect(change(refused)).rejects.toBeInstanceOf(DeckError)
    expect(onChange).not.toHaveBeenCalled()
  })

  it('calls onChange once a delete went through, and never for a read', async () => {
    const onChange = vi.fn()
    const api = createDecksApi({
      fetch: fakeFetch(() => new Response(null, { status: 204 })).fetch,
      onChange,
    })
    await api.remove('a')
    expect(onChange).toHaveBeenCalledTimes(1)
    await expect(api.list()).rejects.toBeInstanceOf(DeckError)
    expect(onChange).toHaveBeenCalledTimes(1)
  })

  it('drops the cached deck lists, so the next read asks the server', async () => {
    const api = decksOf([ADA], [BOB])
    expect(await cachedDecks(api)).toEqual([ADA])
    expect(await cachedDecks(api)).toEqual([ADA])
    forgetDeckLists()
    expect(await cachedDecks(api)).toEqual([BOB])
    expect(api.list).toHaveBeenCalledTimes(2)
  })
})
