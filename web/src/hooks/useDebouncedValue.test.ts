// Goal: a debounced value starts as the first value, follows a change only after it has held still for the wait, and drops a value that was replaced inside the wait.
import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useDebouncedValue } from './useDebouncedValue'

beforeEach(() => vi.useFakeTimers())
afterEach(() => vi.useRealTimers())

describe('useDebouncedValue', () => {
  it('starts as the first value and follows a change after the wait', () => {
    const { result, rerender } = renderHook(({ value }) => useDebouncedValue(value, 400), {
      initialProps: { value: 'a' },
    })
    expect(result.current).toBe('a')
    rerender({ value: 'b' })
    expect(result.current).toBe('a')
    act(() => void vi.advanceTimersByTime(399))
    expect(result.current).toBe('a')
    act(() => void vi.advanceTimersByTime(1))
    expect(result.current).toBe('b')
  })

  it('drops a value that was replaced inside the wait', () => {
    const { result, rerender } = renderHook(({ value }) => useDebouncedValue(value, 400), {
      initialProps: { value: 'a' },
    })
    rerender({ value: 'b' })
    act(() => void vi.advanceTimersByTime(300))
    rerender({ value: 'c' })
    act(() => void vi.advanceTimersByTime(300))
    expect(result.current).toBe('a')
    act(() => void vi.advanceTimersByTime(100))
    expect(result.current).toBe('c')
  })
})
