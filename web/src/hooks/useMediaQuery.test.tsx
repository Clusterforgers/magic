// Goal: the media query hook reports what the window matches now, follows a change of it, stops listening when its component goes or its query changes, and answers its fallback where the browser has no matchMedia.
import { act, renderHook } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { useMediaQuery } from './useMediaQuery'

afterEach(() => {
  vi.unstubAllGlobals()
})

/** A window with `matchMedia`; `set(query, matches)` changes one query and tells whoever listens to it. */
function stubWindow(initial: Record<string, boolean>) {
  const state = new Map(Object.entries(initial))
  const listeners = new Map<string, Set<() => void>>()
  vi.stubGlobal('matchMedia', (query: string) => {
    const set = listeners.get(query) ?? new Set<() => void>()
    listeners.set(query, set)
    return {
      get matches() {
        return state.get(query) ?? false
      },
      addEventListener: (_type: string, listener: () => void) => void set.add(listener),
      removeEventListener: (_type: string, listener: () => void) => void set.delete(listener),
    }
  })
  return {
    set(query: string, matches: boolean) {
      state.set(query, matches)
      act(() => {
        for (const listener of listeners.get(query) ?? []) listener()
      })
    },
    listening: (query: string) => listeners.get(query)?.size ?? 0,
  }
}

describe('useMediaQuery', () => {
  it.each([true, false])('answers its fallback (%s) where there is no matchMedia', (fallback) => {
    const { result } = renderHook(() => useMediaQuery('(min-width: 64rem)', fallback))
    expect(result.current).toBe(fallback)
  })

  it.each([
    [true, true],
    [false, false],
  ])('answers what the window matches (%s), whatever the fallback', (matches, expected) => {
    stubWindow({ '(min-width: 64rem)': matches })
    expect(renderHook(() => useMediaQuery('(min-width: 64rem)', !matches)).result.current).toBe(expected)
  })

  it('follows the window across the query', () => {
    const win = stubWindow({ '(min-width: 64rem)': false })
    const { result } = renderHook(() => useMediaQuery('(min-width: 64rem)', true))
    expect(result.current).toBe(false)
    win.set('(min-width: 64rem)', true)
    expect(result.current).toBe(true)
    win.set('(min-width: 64rem)', false)
    expect(result.current).toBe(false)
  })

  it('listens to its own query only, and stops when its component goes', () => {
    const win = stubWindow({ a: true, b: false })
    const view = renderHook(() => useMediaQuery('a', false))
    expect(win.listening('a')).toBe(1)
    expect(win.listening('b')).toBe(0)
    view.unmount()
    expect(win.listening('a')).toBe(0)
  })

  it('moves to a new query when the query changes', () => {
    const win = stubWindow({ a: true, b: false })
    const { result, rerender } = renderHook(({ query }) => useMediaQuery(query, false), {
      initialProps: { query: 'a' },
    })
    expect(result.current).toBe(true)
    rerender({ query: 'b' })
    expect(result.current).toBe(false)
    expect(win.listening('a')).toBe(0)
    expect(win.listening('b')).toBe(1)
  })
})
