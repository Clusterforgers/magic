// Goal: the shared strip between the boards is one status line of fixed height in every prompt and between prompts: the stack chips, the line that says what the game waits for, who has priority and the ticker; it holds no question, clock or answer, which stand in the decision's home; on a phone the strip is a lower line.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import type { Viewport } from './layout'

const strip = (): HTMLElement => screen.getByRole('region', { name: 'Turn and priority' })
const sheet = (): HTMLElement => must(strip().querySelector<HTMLElement>('[data-slot="shared-sheet"]'))

describe('the strip', () => {
  it('is a cell of the grid between the boards, filled by one line that never grows', () => {
    renderTable({ seats: 2 })
    expect(strip()).toHaveAttribute('data-region', 'shared')
    expect(strip()).toHaveClass('relative', 'z-20', 'min-h-0')
    expect(sheet()).toHaveClass('absolute', 'inset-0', 'overflow-hidden', 'plaque')
  })

  it.each<[string, Viewport]>([
    ['1920x1080', { width: 1920, height: 1080 }],
    ['1366x768', { width: 1366, height: 768 }],
    ['1024x768', { width: 1024, height: 768 }],
    ['390x844', { width: 390, height: 844 }],
  ])('holds no part of a decision at %s: its home does', (_name, viewport) => {
    const table = renderTable({ seats: 2, options: { viewport } })
    table.update(() => ({ pending: { id: 9, decision: { player: 0, kind: { YesNo: { source: 1 } } } } }))
    for (const slot of ['prompt-text', 'prompt-actions', 'response-clock', 'prompt-choices']) {
      expect(strip().querySelector(`[data-slot="${slot}"]`), slot).toBeNull()
    }
    expect(document.querySelectorAll('[data-slot="decision-line"]')).toHaveLength(1)
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
  it('leaves out "You have priority" beside the seat`s own prompt, which its home says', () => {
    renderTable({ seats: 4 })
    expect(strip()).not.toHaveTextContent('has priority')
    expect(strip().querySelector('[data-decision]')).toBeNull()
    expect(document.querySelector('[data-decision="Priority"]')).not.toBeNull()
  })

  it('holds the line that says who the game waits for, as a polite live region, once the seat has answered', () => {
    renderTable({ seats: 4, state: (state) => ({ ...state, pending: null }) })
    const line = must(strip().querySelector<HTMLElement>('[data-slot="waiting-line"]'))
    expect(line).toHaveAttribute('aria-live', 'polite')
    expect(line).toHaveTextContent(/^Waiting for /)
  })

  it('has the stack chips', () => {
    renderTable({ seats: 4 })
    expect(strip().querySelector('[data-slot="stack-chips"]')).not.toBeNull()
  })

  it('drops the key to the combat lines on a phone', () => {
    renderTable({ seats: 2, options: { viewport: { width: 390, height: 844 } } })
    expect(strip().className).toContain('max-sm:[&_[data-slot=combat-key]]:hidden')
    expect(within(strip()).queryByText(/^Your decision/)).toBeNull()
  })

  it('starts beside the pod column, so it never covers your life, and at the edge on a phone', () => {
    const { unmount } = renderTable({ seats: 2, options: { viewport: { width: 1600, height: 1000 } } })
    // The pod column of 168 px and a gap of 8.
    expect(sheet().style.left).toBe('176px')
    unmount()
    renderTable({ seats: 2, options: { viewport: { width: 390, height: 844 } } })
    expect(sheet().style.left).toBe('')
  })
})
