// Goal: the deck pane's look (pictures or the dense list) starts as pictures, remembers the choice in storage, and falls back to pictures when storage cannot be read or written.
import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { DECK_LOOK_KEY, loadDeckLook, useDeckLook } from './deckLook'

beforeEach(() => window.localStorage.clear())
afterEach(() => vi.restoreAllMocks())

describe('the deck look', () => {
  it('is pictures until a choice was kept, and reads a kept list', () => {
    expect(loadDeckLook()).toBe('pictures')
    window.localStorage.setItem(DECK_LOOK_KEY, 'list')
    expect(loadDeckLook()).toBe('list')
  })

  it('reads anything else as pictures', () => {
    window.localStorage.setItem(DECK_LOOK_KEY, 'grid')
    expect(loadDeckLook()).toBe('pictures')
  })

  it('stores a choice made through the hook and starts the next page with it', () => {
    const first = renderHook(() => useDeckLook())
    act(() => first.result.current[1]('list'))
    expect(first.result.current[0]).toBe('list')
    expect(window.localStorage.getItem(DECK_LOOK_KEY)).toBe('list')
    expect(renderHook(() => useDeckLook()).result.current[0]).toBe('list')
  })

  it('works for the page when storage throws', () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('blocked')
    })
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('blocked')
    })
    const { result } = renderHook(() => useDeckLook())
    expect(result.current[0]).toBe('pictures')
    act(() => result.current[1]('list'))
    expect(result.current[0]).toBe('list')
  })
})
