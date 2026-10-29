// Goal: the loader checks only the saved decks whose summary holds a card that is not Verified, four at a time, names a deck whose check failed without losing the others, tells a ended session and a server without decks apart from other failures, reads one deck in one call, and asks again on reload.
import { act, renderHook, waitFor } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { DeckError } from '@/net/decks'
import type { DeckReport } from '@/protocol/generated'
import { counts, deckIn, reportCard, reportOf, summary } from '../fakeDecks'
import { FIXTURE_DECKS, fixtureDecksApi } from './fixture'
import { useUnvalidated } from './useUnvalidated'

const flaggedItem = (id: string) =>
  deckIn('manual_play', {
    id,
    name: `Deck ${id}`,
    summary: summary({ counts: counts({ manual: 2, verified: 98 }) }),
  })

describe('useUnvalidated, every deck', () => {
  it('checks the decks that hold a flagged card and not the one that runs whole', async () => {
    const api = fixtureDecksApi()
    const { result } = renderHook(() => useUnvalidated(api, { kind: 'all' }))
    await waitFor(() => expect(result.current.state.status).toBe('ready'))
    expect(api.asked.sort()).toEqual(['check deck-atraxa', 'check deck-chaos', 'check deck-izzet', 'list'])
    const state = result.current.state
    if (state.status !== 'ready') throw new Error('not ready')
    expect(state.decks.map((deck) => deck.name)).toEqual([
      'Atraxa Superfriends',
      'Izzet Spells',
      'Chaos Brew With A Rather Long Name To Wrap',
    ])
    expect(state.saved).toBe(4)
    expect(state.failed).toEqual([])
  })

  it('runs four checks at once and counts the ones done while it waits', async () => {
    let running = 0
    let most = 0
    const finish: Array<() => void> = []
    const items = Array.from({ length: 6 }, (_, index) => flaggedItem(`d${index + 1}`))
    const api = {
      list: () => Promise.resolve(items),
      get: () => Promise.reject(new Error('unused')),
      check: () =>
        new Promise<DeckReport>((resolve) => {
          running += 1
          most = Math.max(most, running)
          finish.push(() => {
            running -= 1
            resolve(reportOf([reportCard('Zed', { support: 'manual' })]))
          })
        }),
    }
    const { result } = renderHook(() => useUnvalidated(api, { kind: 'all' }))
    await waitFor(() => expect(finish).toHaveLength(4))
    expect(result.current.state).toEqual({ status: 'loading', done: 0, total: 6 })
    await act(async () => {
      finish[0]?.()
      await Promise.resolve()
    })
    await waitFor(() => expect(finish).toHaveLength(5))
    expect(result.current.state).toEqual({ status: 'loading', done: 1, total: 6 })
    await act(async () => {
      for (const done of finish.slice(1)) done()
      await Promise.resolve()
    })
    await waitFor(() => expect(finish).toHaveLength(6))
    await act(async () => {
      finish[5]?.()
      await Promise.resolve()
    })
    await waitFor(() => expect(result.current.state.status).toBe('ready'))
    expect(most).toBe(4)
  })

  it('names a deck whose check failed and lists the others', async () => {
    const api = fixtureDecksApi({ checkFails: ['deck-izzet'] })
    const { result } = renderHook(() => useUnvalidated(api, { kind: 'all' }))
    await waitFor(() => expect(result.current.state.status).toBe('ready'))
    const state = result.current.state
    if (state.status !== 'ready') throw new Error('not ready')
    expect(state.failed).toEqual([{ id: 'deck-izzet', name: 'Izzet Spells' }])
    expect(state.decks.map((deck) => deck.id)).toEqual(['deck-atraxa', 'deck-chaos'])
  })

  it.each([
    ['an ended session', 'unauthenticated', { status: 'failed', sessionEnded: true }],
    ['a server without decks', 'decks_unavailable', { status: 'unavailable' }],
    ['a network failure', 'network', { status: 'failed', sessionEnded: false }],
  ] as const)('says %s from the list', async (_, code, expected) => {
    const api = fixtureDecksApi({ listFails: code })
    const { result } = renderHook(() => useUnvalidated(api, { kind: 'all' }))
    await waitFor(() => expect(result.current.state.status).not.toBe('loading'))
    expect(result.current.state).toMatchObject(expected)
  })

  it('stops at once when the session ends during the checks', async () => {
    const api = {
      ...fixtureDecksApi(),
      check: () => Promise.reject(new DeckError('unauthenticated', 401, 'sign in')),
    }
    const { result } = renderHook(() => useUnvalidated(api, { kind: 'all' }))
    await waitFor(() => expect(result.current.state.status).toBe('failed'))
    expect(result.current.state).toMatchObject({ sessionEnded: true })
  })

  it('has nothing to list for an account without decks', async () => {
    const api = fixtureDecksApi({ decks: [] })
    const { result } = renderHook(() => useUnvalidated(api, { kind: 'all' }))
    await waitFor(() => expect(result.current.state.status).toBe('ready'))
    expect(result.current.state).toEqual({ status: 'ready', decks: [], failed: [], saved: 0 })
    expect(api.asked).toEqual(['list'])
  })

  it('asks everything again on reload', async () => {
    const api = fixtureDecksApi()
    const { result } = renderHook(() => useUnvalidated(api, { kind: 'all' }))
    await waitFor(() => expect(result.current.state.status).toBe('ready'))
    act(() => result.current.reload())
    expect(result.current.state.status).toBe('loading')
    await waitFor(() => expect(result.current.state.status).toBe('ready'))
    expect(api.asked.filter((call) => call === 'list')).toHaveLength(2)
  })
})

describe('useUnvalidated, one deck', () => {
  it('reads the deck once, with its name and the report of the moment', async () => {
    const api = fixtureDecksApi()
    const { result } = renderHook(() => useUnvalidated(api, { kind: 'deck', id: 'deck-izzet' }))
    await waitFor(() => expect(result.current.state.status).toBe('ready'))
    expect(api.asked).toEqual(['get deck-izzet'])
    const state = result.current.state
    if (state.status !== 'ready') throw new Error('not ready')
    expect(state.decks).toEqual([
      { id: 'deck-izzet', name: 'Izzet Spells', cards: FIXTURE_DECKS[2]?.report.cards },
    ])
  })

  it('fails with the words for a deck that is gone', async () => {
    const api = fixtureDecksApi()
    const { result } = renderHook(() => useUnvalidated(api, { kind: 'deck', id: 'nope' }))
    await waitFor(() => expect(result.current.state.status).toBe('failed'))
    expect(result.current.state).toMatchObject({
      message: expect.stringContaining('no longer on the server'),
    })
  })
})
