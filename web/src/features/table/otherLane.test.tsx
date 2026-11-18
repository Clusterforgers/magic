//! Goal: on every board the lane of other permanents, set at the right, lays its cards out from its right edge (the first at the right, each next to its left), so a new artifact joins at its left and the cards already there stay in place, and the arrow keys move the way they point; the lands and creatures lanes still run from the left.
import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { renderTable } from '@/test-utils/tableHarness'

const lane = (owner: string, kind: string): HTMLElement =>
  screen.getByRole('list', { name: `${owner}'s ${kind}` })
const cards = (row: HTMLElement): HTMLElement[] => [
  ...row.querySelectorAll<HTMLElement>('button[data-object-id]'),
]

describe('the lane of other permanents', () => {
  it.each([
    ['your board', 'Alice'],
    ["an opponent's board", 'Bo'],
  ])('on %s runs from its right edge', (_where, owner) => {
    renderTable({ seats: 2 })
    expect(lane(owner, 'other permanents')).toHaveClass('flex-row-reverse')
    expect(lane(owner, 'creatures')).not.toHaveClass('flex-row-reverse')
    expect(lane(owner, 'lands')).not.toHaveClass('flex-row-reverse')
  })

  it.each([2, 4, 6])('runs from its right edge at a table of %i seats', (seats) => {
    renderTable({ seats })
    for (const row of screen.getAllByRole('list', { name: /'s other permanents$/ })) {
      expect(row).toHaveClass('flex-row-reverse')
    }
  })

  it('moves left to the next card and right back, the way the arrows point', async () => {
    const table = renderTable({ seats: 2 })
    const row = lane('Alice', 'other permanents')
    const [first, second] = cards(row)
    if (first === undefined || second === undefined) throw new Error('the fixture has two other permanents')
    first.focus()
    await table.user.keyboard('{ArrowLeft}')
    expect(second).toHaveFocus()
    await table.user.keyboard('{ArrowRight}')
    expect(first).toHaveFocus()
  })
})
