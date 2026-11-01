// Goal: the favourites hook loads the account's stars once, flips a star at once and puts it back with a reason when the server refuses, and asks for nothing for a guest.
import { act, renderHook, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { DeckError } from '@/net/decks'
import type { FavouritesApi } from '@/net/favourites'
import { favouriteWords, useFavourites } from './useFavourites'

const api = (over: Partial<FavouritesApi> = {}): FavouritesApi => ({
  list: vi.fn().mockResolvedValue(['a']),
  star: vi.fn().mockResolvedValue(undefined),
  unstar: vi.fn().mockResolvedValue(undefined),
  ...over,
})

describe('useFavourites', () => {
  it('loads the stars when enabled and asks for nothing for a guest', async () => {
    const guest = api()
    const { result: none } = renderHook(() => useFavourites(guest, false))
    expect(guest.list).not.toHaveBeenCalled()
    expect(none.current.codes.size).toBe(0)
    const member = api()
    const { result } = renderHook(() => useFavourites(member, true))
    await waitFor(() => expect(result.current.has('a')).toBe(true))
  })

  it.each([
    ['stars', 'b', 'star', true],
    ['unstars', 'a', 'unstar', false],
  ] as const)('%s %s at once and calls %s', async (_what, code, call, starred) => {
    const server = api()
    const { result } = renderHook(() => useFavourites(server, true))
    await waitFor(() => expect(result.current.has('a')).toBe(true))
    act(() => result.current.toggle(code))
    expect(result.current.has(code)).toBe(starred)
    expect(server[call]).toHaveBeenCalledWith(code)
    await waitFor(() => expect(result.current.isPending(code)).toBe(false))
    expect(result.current.problem).toBeNull()
  })

  it('puts the star back and says why when the server refuses', async () => {
    const server = api({ star: vi.fn().mockRejectedValue(new DeckError('rate_limited', 429, 'slow')) })
    const { result } = renderHook(() => useFavourites(server, true))
    await waitFor(() => expect(result.current.has('a')).toBe(true))
    act(() => result.current.toggle('b'))
    await waitFor(() => expect(result.current.problem).toContain('too often'))
    expect(result.current.has('b')).toBe(false)
    act(() => result.current.clearProblem())
    expect(result.current.problem).toBeNull()
  })

  it('stays silent when the list cannot be had', async () => {
    const server = api({ list: vi.fn().mockRejectedValue(new Error('down')) })
    const { result } = renderHook(() => useFavourites(server, true))
    await waitFor(() => expect(server.list).toHaveBeenCalled())
    expect(result.current.problem).toBeNull()
  })
})

describe('favouriteWords', () => {
  it.each([
    ['not_found', 'no longer on the server'],
    ['unauthenticated', 'Sign in again'],
  ] as const)('explains %s', (code, words) => {
    expect(favouriteWords(new DeckError(code, 400, 'x'), true)).toContain(words)
  })
})
