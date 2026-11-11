// Goal: every caller of the second ticker shares one interval, which runs only while some caller is on and stops with the last one.
import { act, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { TICK_MS, useSecondTick } from './useSecondTick'

afterEach(() => vi.useRealTimers())

function Probe({ on, id }: { on: boolean; id: string }) {
  const now = useSecondTick(on)
  return <span data-testid={id}>{now}</span>
}

const shown = (id: string): number => Number(screen.getByTestId(id).textContent)

describe('useSecondTick', () => {
  it('shares one interval between callers and refreshes each once a second', () => {
    vi.useFakeTimers({ now: 1_000_000 })
    const intervals = vi.spyOn(globalThis, 'setInterval')
    render(
      <>
        <Probe on id="a" />
        <Probe on id="b" />
        <Probe on={false} id="c" />
      </>,
    )
    expect(intervals).toHaveBeenCalledTimes(1)
    act(() => vi.advanceTimersByTime(3 * TICK_MS))
    expect(shown('a')).toBe(1_003_000)
    expect(shown('b')).toBe(1_003_000)
    expect(shown('c')).toBe(1_000_000)
  })

  it('runs no interval while every caller is off, and stops when the last one goes', () => {
    vi.useFakeTimers()
    const intervals = vi.spyOn(globalThis, 'setInterval')
    const cleared = vi.spyOn(globalThis, 'clearInterval')
    const view = render(<Probe on={false} id="a" />)
    expect(intervals).not.toHaveBeenCalled()
    view.rerender(<Probe on id="a" />)
    expect(intervals).toHaveBeenCalledTimes(1)
    view.unmount()
    expect(cleared).toHaveBeenCalledTimes(1)
  })
})
