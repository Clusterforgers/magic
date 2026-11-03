// Goal: the opponents of a three or four seat table are each framed by a border, a seat that left the game shrinks to a thin strip with its name and result and gives its space to the others, and every seat shows its untapped mana sources by colour next to its pool.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { LossReason } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'

describe('the opponents area', () => {
  it.each([
    [3, 2],
    [4, 3],
  ])('frames each of the %s-seat table opponents (%s frames)', (seats, frames) => {
    const { container } = renderTable({ seats })
    expect(container.querySelectorAll('[data-slot="seat-frame"]')).toHaveLength(frames)
    expect(container.querySelectorAll('[data-slot="left-strip"]')).toHaveLength(0)
  })

  it.each<[string, LossReason, string]>([
    ['zero life', 'ZeroLife', 'out of life'],
    ['a concession', 'Conceded', 'conceded'],
  ])('shrinks a seat that left by %s to a strip and frames the others', (_, reason, word) => {
    const { container } = renderTable({
      seats: 4,
      state: (state) => ({
        ...state,
        view: {
          ...must(state.view),
          players: must(state.view).players.map((player) =>
            player.seat === 2 ? { ...player, lost: reason } : player,
          ),
        },
      }),
    })
    const strip = must(container.querySelector<HTMLElement>('[data-slot="left-strip"]'))
    expect(strip).toHaveAttribute('data-seat-area', '2')
    expect(strip.textContent?.toLowerCase()).toContain(word)
    expect(container.querySelectorAll('[data-slot="seat-frame"]')).toHaveLength(2)
    expect(container.querySelector('[data-seat-area="2"][data-slot="seat-area"]')).toBeNull()
  })
})

describe('untapped mana sources', () => {
  it('shows a group for a seat with untapped sources, counted by colour', () => {
    renderTable({ seats: 2 })
    const groups = screen.getAllByRole('group', { name: 'Untapped mana sources' })
    expect(groups.length).toBeGreaterThan(0)
    for (const group of groups) {
      expect(within(group).getAllByText(/untapped .* sources?/).length).toBeGreaterThan(0)
    }
  })
})
