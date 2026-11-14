// Goal: a grid's opponent board is enlarged over the opponents' row from its pod's button and closed by the button, Escape, a press outside the row or a decision other than priority, never hiding what a prompt asks for; a duel has no such button.
import { fireEvent, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { decisionFor } from '@/test-utils/decisions'
import { renderTable } from '@/test-utils/tableHarness'

const enlarged = (): HTMLElement | null => screen.queryByRole('region', { name: /'s board, enlarged$/ })
const boards = (container: HTMLElement): string[] =>
  [...container.querySelectorAll('[data-battlefield-seat]')].map(
    (board) => board.getAttribute('data-battlefield-seat') ?? '',
  )

function zoomCy() {
  const table = renderTable({ seats: 4 })
  fireEvent.click(screen.getByRole('button', { name: "Enlarge Cy's board" }))
  return table
}

describe('the enlarged board', () => {
  it('draws the one seat over the row, with yours, and the button turns back', () => {
    const { container } = zoomCy()
    expect(enlarged()).toHaveAccessibleName("Cy's board, enlarged")
    expect(boards(container).sort()).toEqual(['0', '2'])
    const back = screen.getByRole('button', { name: 'Back to every board' })
    expect(back).toHaveAttribute('aria-pressed', 'true')
    fireEvent.click(back)
    expect(enlarged()).toBeNull()
    expect(boards(container).sort()).toEqual(['0', '1', '2', '3'])
  })

  it('closes on Escape', () => {
    zoomCy()
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(enlarged()).toBeNull()
  })

  it('closes on a press outside the row and stays on a press inside it', () => {
    const { container } = zoomCy()
    fireEvent.pointerDown(must(enlarged()))
    expect(enlarged()).not.toBeNull()
    fireEvent.pointerDown(must(container.querySelector('[data-region="hand"]')))
    expect(enlarged()).toBeNull()
  })

  it.each([
    ['priority', 'Priority', false],
    ['a choice of targets', 'ChooseTargets', true],
    ['blockers', 'DeclareBlockers', true],
  ] as const)('on %s it closes: %s', (_, kind, closes) => {
    const { update } = zoomCy()
    update(() => ({ pending: { id: 40, decision: decisionFor(kind) } }))
    expect(enlarged() === null).toBe(closes)
  })

  it('has no button at a duel', () => {
    renderTable({ seats: 2 })
    expect(screen.queryByRole('button', { name: /^Enlarge / })).toBeNull()
  })
})

function must<T>(value: T | null): T {
  if (value === null) throw new Error('missing')
  return value
}
