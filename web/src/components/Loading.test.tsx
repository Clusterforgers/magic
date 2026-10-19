// Goal: a waiting region tells a screen reader at once and draws its skeleton only after 300 ms, so a quick answer never flashes grey shapes.
import { act, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { SKELETON_DELAY_MS } from '@/hooks/useAfterDelay'
import { Loading } from './Loading'

beforeEach(() => {
  vi.useFakeTimers()
})

afterEach(() => {
  vi.useRealTimers()
})

describe('Loading', () => {
  it('announces what is loading right away and draws nothing else', () => {
    render(
      <Loading label="Loading the decks…">
        <div data-testid="skeleton" />
      </Loading>,
    )
    expect(screen.getByRole('status')).toHaveTextContent('Loading the decks…')
    expect(screen.getByRole('status')).toHaveAttribute('aria-busy', 'true')
    expect(screen.queryByTestId('skeleton')).toBeNull()
  })

  it('shows the skeleton from 300 ms on, and not a moment before', () => {
    expect(SKELETON_DELAY_MS).toBe(300)
    render(
      <Loading label="Loading">
        <div data-testid="skeleton" />
      </Loading>,
    )
    act(() => vi.advanceTimersByTime(299))
    expect(screen.queryByTestId('skeleton')).toBeNull()
    act(() => vi.advanceTimersByTime(1))
    expect(screen.getByTestId('skeleton')).toBeInTheDocument()
  })

  it('draws no skeleton, and leaves no timer behind, when the answer comes first', () => {
    const { unmount } = render(
      <Loading label="Loading">
        <div data-testid="skeleton" />
      </Loading>,
    )
    unmount()
    expect(vi.getTimerCount()).toBe(0)
    act(() => vi.advanceTimersByTime(1000))
    expect(screen.queryByTestId('skeleton')).toBeNull()
  })
})
