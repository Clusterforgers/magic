// Goal: an answer the server has not followed within 400 ms shows a quiet working mark in the strip, in every layout, and the server's next word takes it away; a quicker reply never flashes it.
import { act, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { Answer } from '@/protocol/generated'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from './dev/fixture'
import { withBoss } from './dev/devGame'
import type { Fixture } from './dev/fixture'
import type { Viewport } from './layout'
import { WORKING_AFTER_MS } from './SharedStrip'

const mark = (): HTMLElement | null => document.querySelector('[data-slot="working-mark"]')
const answered: Answer = { Priority: 'Pass' }

beforeEach(() => {
  vi.useFakeTimers()
})
afterEach(() => {
  vi.useRealTimers()
})

describe('the working mark', () => {
  it.each<[string, () => Fixture, Viewport]>([
    ['1v1', () => buildFixture({ seats: 2 }), { width: 1400, height: 900 }],
    ['a grid of four', () => buildFixture({ seats: 4 }), { width: 1400, height: 900 }],
    ['six seats', () => buildFixture({ seats: 6 }), { width: 1920, height: 1080 }],
    ['a boss fight', () => withBoss(buildFixture({ seats: 4 }), 0), { width: 1400, height: 900 }],
    ['a phone', () => buildFixture({ seats: 2 }), { width: 390, height: 844 }],
  ])(
    'shows past 400 ms of silence after an answer at %s, and goes with the next word',
    (_name, fixture, viewport) => {
      const h = renderTable({
        fixture: fixture(),
        options: { viewport },
        state: (state) => ({ ...state, pending: null }),
      })
      h.update(() => ({ answered }))
      act(() => vi.advanceTimersByTime(WORKING_AFTER_MS - 1))
      expect(mark()).toBeNull()
      act(() => vi.advanceTimersByTime(1))
      expect(mark()).not.toBeNull()
      expect(screen.getByRole('status', { name: /waiting for the server/i })).toBe(mark())
      h.update(() => ({ answered: null }))
      expect(mark()).toBeNull()
    },
  )

  it('never flashes when the server answers within 400 ms', () => {
    const h = renderTable({ seats: 2, state: (state) => ({ ...state, pending: null }) })
    h.update(() => ({ answered }))
    act(() => vi.advanceTimersByTime(300))
    h.update(() => ({ answered: null }))
    act(() => vi.advanceTimersByTime(1000))
    expect(mark()).toBeNull()
  })

  it('is quiet: a small ring that turns only when motion is welcome', () => {
    const h = renderTable({ seats: 2, state: (state) => ({ ...state, pending: null }) })
    h.update(() => ({ answered }))
    act(() => vi.advanceTimersByTime(WORKING_AFTER_MS))
    const ring = mark()?.querySelector('[aria-hidden="true"]')
    expect(ring).toHaveClass('size-3', 'motion-safe:animate-spin')
    expect(ring).not.toHaveClass('animate-spin')
  })

  it('is not drawn beside a prompt the seat must answer', () => {
    const h = renderTable({ seats: 2 })
    h.update(() => ({ answered }))
    act(() => vi.advanceTimersByTime(1000))
    expect(mark()).toBeNull()
  })
})
