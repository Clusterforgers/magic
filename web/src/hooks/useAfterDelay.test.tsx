// Goal: the delay hook is true only after its flag has stayed true for the whole wait, starts the wait again each time the flag comes back, and takes the wait it is given.
import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { SKELETON_DELAY_MS, useAfterDelay } from './useAfterDelay'

beforeEach(() => {
  vi.useFakeTimers()
})

afterEach(() => {
  vi.useRealTimers()
})

describe('useAfterDelay', () => {
  it('waits 300 ms by default', () => {
    const { result } = renderHook(() => useAfterDelay(true))
    act(() => vi.advanceTimersByTime(SKELETON_DELAY_MS - 1))
    expect(result.current).toBe(false)
    act(() => vi.advanceTimersByTime(1))
    expect(result.current).toBe(true)
  })

  it('never turns true while the flag is false', () => {
    const { result } = renderHook(() => useAfterDelay(false))
    act(() => vi.advanceTimersByTime(5000))
    expect(result.current).toBe(false)
    expect(vi.getTimerCount()).toBe(0)
  })

  it('turns false the moment the flag does, and counts again from zero when it returns', () => {
    const { result, rerender } = renderHook(({ active }) => useAfterDelay(active, 100), {
      initialProps: { active: true },
    })
    act(() => vi.advanceTimersByTime(100))
    expect(result.current).toBe(true)
    rerender({ active: false })
    expect(result.current).toBe(false)
    rerender({ active: true })
    expect(result.current).toBe(false)
    act(() => vi.advanceTimersByTime(99))
    expect(result.current).toBe(false)
    act(() => vi.advanceTimersByTime(1))
    expect(result.current).toBe(true)
  })

  it('does not reach true when the flag drops before the wait is over', () => {
    const { result, rerender } = renderHook(({ active }) => useAfterDelay(active, 100), {
      initialProps: { active: true },
    })
    act(() => vi.advanceTimersByTime(60))
    rerender({ active: false })
    act(() => vi.advanceTimersByTime(500))
    expect(result.current).toBe(false)
  })
})
