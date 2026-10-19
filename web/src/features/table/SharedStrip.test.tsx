// Goal: the shared strip between the boards holds the stack chips, the prompt or the line that says what the game waits for, and who has priority; a prompt that needs more than the strip grows into a sheet (at most 40 % of the window, over your board and never over the hand dock or the controls) and scrolls inside itself past that; on a phone the strip is a line.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { regionSizes } from './layout'
import type { Viewport } from './layout'

const strip = (): HTMLElement => screen.getByRole('region', { name: 'Turn and priority' })
const sheet = (): HTMLElement => must(strip().querySelector<HTMLElement>('[data-slot="shared-sheet"]'))

describe('the strip and its sheet', () => {
  it('is a cell of the grid with the stack above the boards, and a sheet that grows from its top edge', () => {
    renderTable({ seats: 2 })
    expect(strip()).toHaveAttribute('data-region', 'shared')
    expect(strip()).toHaveClass('relative', 'z-20', 'min-h-0')
    expect(sheet()).toHaveClass('absolute', 'inset-x-0', 'top-0', 'min-h-full')
  })

  it.each<[string, Viewport]>([
    ['1920x1080', { width: 1920, height: 1080 }],
    ['1366x768', { width: 1366, height: 768 }],
    ['1280x720', { width: 1280, height: 720 }],
    ['1024x768', { width: 1024, height: 768 }],
    ['390x844', { width: 390, height: 844 }],
  ])('lets the sheet grow to the most the layout allows at %s', (_name, viewport) => {
    renderTable({ seats: 2, options: { viewport } })
    const most = Math.floor(regionSizes(viewport).sheet)
    expect(strip().style.getPropertyValue('--sheet-max')).toBe(`${most}px`)
    expect(sheet()).toHaveClass('max-h-(--sheet-max)')
    // 40 % of the window at the most, and never further than the own board.
    expect(most).toBeLessThanOrEqual(Math.floor(viewport.height * 0.4))
  })

  it('scrolls inside itself, with the edge fade, when the prompt is taller than the sheet may be', () => {
    renderTable({ seats: 2 })
    expect(sheet()).toHaveClass('fade-scroll')
  })

  it('is stacked above the lines of combat and below the hand dock and the controls', () => {
    const { container } = renderTable({ seats: 2 })
    expect(strip()).toHaveClass('z-20')
    for (const name of ['hand', 'controls']) {
      expect(must(container.querySelector(`[data-area="${name}"]`))).toHaveClass('z-30')
    }
  })
})

describe('what it says', () => {
  it('holds the prompt this seat must answer, and who has priority', () => {
    renderTable({ seats: 4 })
    expect(within(strip()).getByText(/^Your decision: Priority/)).toBeInTheDocument()
    expect(strip()).toHaveTextContent('Alice has priority · 1 passed')
    expect(strip().querySelector('[data-decision="Priority"]')).not.toBeNull()
  })

  it('holds the line that says who the game waits for, as a polite live region, once the seat has answered', () => {
    renderTable({ seats: 4, state: (state) => ({ ...state, pending: null }) })
    const line = must(strip().querySelector<HTMLElement>('[data-slot="decision-line"]'))
    expect(line).toHaveAttribute('aria-live', 'polite')
    expect(line).toHaveTextContent(/^Waiting for /)
  })

  it('has the stack chips beside the prompt', () => {
    renderTable({ seats: 4 })
    expect(strip().querySelector('[data-slot="stack-chips"]')).not.toBeNull()
  })

  it('hides the title of the prompt and the key to the combat lines on a phone, where the strip is a line', () => {
    renderTable({ seats: 2, options: { viewport: { width: 390, height: 844 } } })
    expect(strip().className).toContain('max-sm:[&_[data-slot=prompt-title]]:hidden')
    expect(strip().className).toContain('max-sm:[&_[data-slot=combat-key]]:hidden')
  })
})
