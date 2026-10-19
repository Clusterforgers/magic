// Goal: a seat tile of the focus strip is two lines high and says a seat's name, life, hand, library and creatures at a glance, with the turn, priority and elimination as small marks, named in full for a screen reader, and shows that seat's board when pressed.
import { screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from './dev/fixture'
import { SeatTile } from './SeatTile'

const tile = (seat: number): HTMLElement =>
  must(document.querySelector<HTMLElement>(`[data-tile-seat="${seat}"]`))

function show(seat: number, focused = false, onFocusSeat = vi.fn()) {
  const table = renderTable({
    fixture: buildFixture({ seats: 5, permanents: 6 }),
    ui: <SeatTile seat={seat} focused={focused} onFocusSeat={onFocusSeat} />,
  })
  return { ...table, onFocusSeat }
}

describe('the tile', () => {
  it('has the name and life on one line and the counts on the next', () => {
    show(2)
    const lines = [...tile(2).children]
    expect(lines).toHaveLength(2)
    expect(lines[0]).toHaveTextContent('Cy')
    expect(lines[0]?.querySelector('[data-slot="life"]')).toHaveTextContent('36')
    expect(lines[1]).toHaveTextContent('Hand 7')
    expect(lines[1]).toHaveTextContent('Library 46')
    expect(lines[1]).toHaveTextContent(/\d+ creatures/)
  })

  it('fills its cell, so the strip sets its height', () => {
    show(2)
    expect(tile(2)).toHaveClass('h-full', 'min-w-44', 'flex-1')
  })

  it('marks the active turn and priority as small badges and says so in its name', () => {
    show(1)
    expect(tile(1)).toHaveTextContent('Active turn')
    expect(tile(1)).toHaveAccessibleName("Show Bo's board. Life 38, 6 cards in hand, active turn")
  })

  it('marks an eliminated seat in words, in its name and by the red edge', () => {
    const table = renderTable({
      fixture: buildFixture({ seats: 5, permanents: 2 }),
      ui: <SeatTile seat={3} focused={false} onFocusSeat={() => undefined} />,
      state: (state) => {
        const view = clone(must(state.view))
        must(view.players[3]).lost = 'ZeroLife'
        return { ...state, view }
      },
    })
    expect(table.container).toBeTruthy()
    expect(tile(3)).toHaveTextContent('Eliminated')
    expect(tile(3)).toHaveClass('border-destructive')
    expect(tile(3)).toHaveAccessibleName(expect.stringContaining('eliminated'))
  })

  it('says it is shown, and is the current item of the strip, while its board is the large one', () => {
    show(2, true)
    expect(tile(2)).toHaveAttribute('aria-current', 'true')
    expect(tile(2)).toHaveAccessibleName(expect.stringContaining('shown now'))
    expect(tile(2)).toHaveClass('ring-primary')
  })

  it('asks for its seat when pressed, with the mouse or the keyboard', async () => {
    const { user, onFocusSeat } = show(4)
    await user.click(tile(4))
    expect(onFocusSeat).toHaveBeenCalledWith(4)
    await user.keyboard('{Enter}')
    expect(onFocusSeat).toHaveBeenCalledTimes(2)
    expect(within(document.body).getAllByRole('button')).toHaveLength(1)
    expect(screen.getByRole('button')).toBe(tile(4))
  })
})
