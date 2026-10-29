// Goal: a seat tile of the focus strip is two lines high and says a seat's name with its dot, its life as the largest figure, and chips for hand, library, graveyard, exile, and the command zone when it holds a card, each with an accessible name that the tile's own name repeats in words; it carries the turn marks as `data-state`, marks an elimination in words, draws mana only while some floats, and shows that seat's board when pressed.
import { screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from './dev/fixture'
import { SeatTile } from './SeatTile'
import type { PlayState } from '@/play/store'
import type { StateView } from '@/protocol/generated'

const tile = (seat: number): HTMLElement =>
  must(document.querySelector<HTMLElement>(`[data-tile-seat="${seat}"]`))

function edited(edit: (view: StateView) => void): (state: PlayState) => PlayState {
  return (state) => {
    const view = clone(must(state.view))
    edit(view)
    return { ...state, view }
  }
}

function show(
  seat: number,
  focused = false,
  onFocusSeat = vi.fn(),
  edit: (view: StateView) => void = () => undefined,
) {
  const table = renderTable({
    fixture: buildFixture({ seats: 5, permanents: 6 }),
    ui: <SeatTile seat={seat} focused={focused} onFocusSeat={onFocusSeat} />,
    state: edited(edit),
  })
  return { ...table, onFocusSeat }
}

describe('the tile', () => {
  it('has the dot, name and life on one line and the chips on the next', () => {
    show(2)
    const lines = [...tile(2).children]
    expect(lines).toHaveLength(2)
    expect(lines[0]).toHaveTextContent('Cy')
    expect(lines[0]?.querySelector('[data-slot="seat-dot"]')).toHaveAttribute('aria-hidden', 'true')
    expect(lines[0]?.querySelector('[data-slot="life"]')).toHaveTextContent('36')
    expect(lines[0]?.querySelector('[data-slot="life"]')).toHaveClass('text-xl')
    expect(lines[1]).toBeTruthy()
  })

  it.each([
    ['Hand 7 cards', '7'],
    ['Library 46 cards', '46'],
    ['Graveyard 5 cards', '5'],
    ['Exile 1 card', '1'],
  ])('shows the chip "%s" with its count', (name, count) => {
    show(2)
    const chip = within(tile(2)).getByRole('img', { name })
    expect(chip).toHaveTextContent(count)
    expect(chip.parentElement).toBe(tile(2).children[1])
  })

  it('draws the command zone chip only once the zone holds a card', () => {
    show(1)
    expect(within(tile(1)).getByRole('img', { name: 'Command zone 1 card' })).toHaveTextContent('1')
    document.body.innerHTML = ''
    show(1, false, vi.fn(), (view) => {
      must(view.players[1]).command = { count: 0, cards: [] }
    })
    expect(within(tile(1)).queryByRole('img', { name: /^Command zone/ })).toBeNull()
  })

  it('fills its cell, so the strip sets its height', () => {
    show(2)
    expect(tile(2)).toHaveClass('h-full', 'min-w-44', 'flex-1')
  })

  it('writes everything it shows in its name', () => {
    show(1, true)
    expect(tile(1)).toHaveAccessibleName(
      "Show Bo's board. Life 38, 6 cards in hand, 47 in library, 5 in graveyard, 1 in exile, 1 in the command zone, Atraxa, Praetors’ Voice: cast 2 times from the command zone, tax 4 mana, 6 of 21 commander damage from Tymna the Weaver, active turn, shown now",
    )
  })

  it.each([
    [1, 'active', 'active turn'],
    [0, 'priority', 'has priority'],
  ])('marks seat %i with data-state %s and says so in its name, with no badge', (seat, state, words) => {
    show(seat)
    expect(tile(seat)).toHaveAttribute('data-state', state)
    expect(tile(seat)).toHaveAccessibleName(expect.stringContaining(words))
    expect(tile(seat)).not.toHaveTextContent(/Active turn|Priority/)
  })

  it('lists both marks as tokens, and none for a seat that has neither', () => {
    show(1, false, vi.fn(), (view) => {
      must(view.priority).holder = 1
    })
    expect(tile(1)).toHaveAttribute('data-state', 'active priority')
    document.body.innerHTML = ''
    show(3)
    expect(tile(3)).not.toHaveAttribute('data-state')
  })

  it('draws mana only while some floats', () => {
    show(2)
    expect(within(tile(2)).queryByRole('group', { name: 'Mana pool' })).toBeNull()
    document.body.innerHTML = ''
    show(2, false, vi.fn(), (view) => {
      must(view.players[2]).mana = { counts: [0, 2, 0, 0, 0, 0] }
    })
    expect(within(tile(2)).getByRole('group', { name: 'Mana pool' })).toHaveTextContent('2 blue')
  })

  it('writes no placeholder text', () => {
    show(2)
    expect(tile(2).textContent).not.toMatch(/not tracked|commander damage|n\/a|empty/i)
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

  it('asks for its seat when pressed, with the mouse or the keyboard, and holds no button of its own', async () => {
    const { user, onFocusSeat } = show(4)
    await user.click(tile(4))
    expect(onFocusSeat).toHaveBeenCalledWith(4)
    await user.keyboard('{Enter}')
    expect(onFocusSeat).toHaveBeenCalledTimes(2)
    expect(within(document.body).getAllByRole('button')).toHaveLength(1)
    expect(screen.getByRole('button')).toBe(tile(4))
  })
})
