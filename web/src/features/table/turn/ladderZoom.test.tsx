// Goal: under a coarse pointer the step ladder's stop marks hit at 44 px without widening the top bar: the top bar keeps its small marks as one button that opens the ladder below it, where (as in the stops popover) each mark is a 44 px button spaced so none lies over its neighbour; a fine pointer keeps the marks in the bar as buttons.
import { screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { Step, StopSide } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import type { Viewport } from '../layout'

/** A window whose primary pointer is coarse (`coarse`) or fine. */
function pointer(coarse: boolean): void {
  vi.stubGlobal('matchMedia', (query: string) => ({
    matches: coarse && query.includes('pointer: coarse'),
    media: query,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
  }))
}

function ladder(viewport: Viewport, setStop = vi.fn((_side: StopSide, _step: Step) => true)) {
  const table = renderTable({
    options: { viewport },
    session: { setStop },
    state: (state) => {
      const view = clone(must(state.view))
      view.turn = { ...view.turn, step: 'Main1', active: 1 }
      return { ...state, view, stops: [{ side: 'Opponent', step: 'End', keep: true }], status: 'open' }
    },
  })
  return { ...table, setStop }
}

const bar = (): HTMLElement => must(document.querySelector<HTMLElement>('[data-region="bar"]'))
const stops = (root: ParentNode = document): HTMLElement[] => [
  ...root.querySelectorAll<HTMLElement>('button[data-slot="stop"]'),
]

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('a coarse pointer at the top bar', () => {
  beforeEach(() => pointer(true))

  it.each<[string, Viewport]>([
    ['1024', { width: 1024, height: 768 }],
    ['1400', { width: 1400, height: 900 }],
  ])(
    'keeps the small marks in the bar as one button, which opens the ladder with 44 px stops, at %s px',
    async (_name, viewport) => {
      const { user, setStop } = ladder(viewport)
      // The bar draws the same 20 px marks as before, so it is no wider; they are a picture inside one button.
      expect(stops(bar())).toEqual([])
      const zoom = within(bar()).getByRole('button', { name: /^Stops on .*: set stops$/ })
      expect(zoom).toHaveClass('hit-area')
      const marks = zoom.querySelectorAll('[data-slot="step-ladder"] li[data-step]')
      expect(marks.length).toBeGreaterThanOrEqual(10)
      for (const mark of marks) expect(mark).toHaveClass('w-5', 'h-3')
      // A held stop still shows on the picture.
      expect(zoom.querySelector('li[data-step="End"]')).toHaveAttribute('data-hold', 'lasting')

      await user.click(zoom)
      const zoomed = must(document.querySelector<HTMLElement>('[data-slot="step-ladder"][data-place="zoom"]'))
      expect(bar().contains(zoomed)).toBe(false)
      expect(stops(zoomed)).toHaveLength(10)
      for (const button of stops(zoomed)) {
        expect(button).toHaveClass('pointer-coarse:h-11', 'pointer-coarse:w-11')
        // 20 px of mark and 12 px of margin each side: 44 px per mark, so no button lies over the next.
        expect(button.parentElement).toHaveClass('w-5', 'pointer-coarse:mx-3')
      }
      await user.click(must(zoomed.querySelector<HTMLElement>('button[data-step="Draw"]')))
      expect(setStop).toHaveBeenCalledExactlyOnceWith('Opponent', 'Draw', true, false)
    },
  )
})

describe('a fine pointer at the top bar', () => {
  beforeEach(() => pointer(false))

  it('keeps the marks in the bar as the stop buttons, at their small spacing', () => {
    ladder({ width: 1400, height: 900 })
    expect(stops(bar())).toHaveLength(10)
    expect(within(bar()).queryByRole('button', { name: /set stops$/ })).toBeNull()
    for (const button of stops(bar())) expect(button.parentElement).not.toHaveClass('pointer-coarse:mx-3')
  })
})

describe('the stops popover below 1024 px', () => {
  beforeEach(() => pointer(true))

  it('spaces its marks for a finger too', async () => {
    const { user } = ladder({ width: 768, height: 1024 })
    await user.click(screen.getByRole('button', { name: /^Stops/ }))
    const popover = await screen.findByRole('dialog', { name: 'Stops and Full Control' })
    expect(stops(popover)).toHaveLength(10)
    for (const button of stops(popover)) expect(button).toHaveClass('pointer-coarse:w-11')
  })
})
