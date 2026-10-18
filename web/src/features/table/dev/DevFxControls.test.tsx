// Goal: the dev effect buttons play their move on the table once for every press and show its effect (which ends on its own), and a move named in the address plays once, FIRST_MOVE_MS after the page loads, and never after the page is gone.
import { act, fireEvent, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { GameTable } from '@/features/table/GameTable'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { DEV_FX } from '../devSearch'
import type { DevFx } from '../devSearch'
import { FX_MS } from '../fx/model'
import { DevFxControls, FIRST_MOVE_MS } from './DevFxControls'

beforeEach(() => {
  vi.useFakeTimers()
})

afterEach(() => {
  vi.useRealTimers()
})

function mount(initial?: DevFx, effects = true) {
  const table = renderTable({
    options: { effects },
    ui: (
      <>
        <GameTable />
        <DevFxControls initial={initial} />
      </>
    ),
  })
  const seq = (): number => must(table.store.getState().seq)
  const press = (name: string): void => {
    fireEvent.click(screen.getByRole('button', { name }))
  }
  const advance = (ms: number): void => {
    act(() => {
      vi.advanceTimersByTime(ms)
    })
  }
  return { ...table, seq, press, advance }
}

const decorated = (container: HTMLElement): number => container.querySelectorAll('[data-fx]').length

describe('the buttons', () => {
  it('are one group with one button for each move', () => {
    mount()
    const group = screen.getByRole('group', { name: 'Table effects (development)' })
    expect([...group.querySelectorAll('button')].map((button) => button.textContent)).toEqual([...DEV_FX])
  })

  it.each(DEV_FX)('%s plays its move once for a press and shows its effect', (kind) => {
    const { container, press, seq } = mount()
    const before = seq()
    press(kind)
    expect(seq()).toBeGreaterThan(before)
    expect(decorated(container) + container.querySelectorAll('li[data-ghost]').length).toBeGreaterThan(0)
  })

  it('end the effect on their own, and play it again on the next press', () => {
    const { container, press, advance } = mount()
    press('life')
    expect(decorated(container)).toBeGreaterThan(0)
    advance(FX_MS.healed)
    expect(decorated(container)).toBe(0)
    press('life')
    expect(decorated(container)).toBeGreaterThan(0)
  })

  it('play the move on the real table: the life totals change and say so', () => {
    const { container, press } = mount()
    const total = (seat: number): string =>
      must(container.querySelector(`section[data-seat="${seat}"] [data-slot="life"]`)).textContent ?? ''
    expect(total(1)).toContain('38')
    press('life')
    expect(total(1)).toContain('33')
    expect(total(0)).toContain('44')
  })

  it('draw nothing but the move while effects are off', () => {
    const { container, press, seq } = mount(undefined, false)
    const before = seq()
    press('damage')
    expect(seq()).toBeGreaterThan(before)
    expect(decorated(container)).toBe(0)
  })
})

describe('a move named in the address', () => {
  it('plays once, after the page has loaded, and then waits for the buttons', () => {
    const { seq, advance } = mount('death')
    const before = seq()
    advance(FIRST_MOVE_MS - 1)
    expect(seq()).toBe(before)
    advance(1)
    const after = seq()
    expect(after).toBeGreaterThan(before)
    advance(5000)
    expect(seq()).toBe(after)
  })

  it('plays nothing when no move is named', () => {
    const { seq, advance } = mount()
    const before = seq()
    advance(5000)
    expect(seq()).toBe(before)
  })

  it('does not play when the page is gone before its time', () => {
    const { seq, advance, unmount } = mount('death')
    const before = seq()
    advance(FIRST_MOVE_MS - 1)
    unmount()
    advance(5000)
    expect(seq()).toBe(before)
    expect(vi.getTimerCount()).toBe(0)
  })
})
