// Goal: the Commands panel opens with a read-only "To do by hand" group that lists each of the seat's due lines as its card and its quoted words, in the order they came due, and draws nothing while none is due; Done forgets them and sends nothing, and a pass of the seat's priority that the server reports clears them.
import { screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ManualAction } from '@/protocol/generated'
import { renderTable } from '@/test-utils/tableHarness'
import { CommandsPanel } from './CommandsPanel'

const WIDE = { width: 1600, height: 1000 }
const LINES = [
  { card: 'Odd Watcher', text: 'Whenever a creature dies, juggle.' },
  { card: 'Odd Watcher', text: 'Whenever a creature dies, juggle.' },
  { card: 'Riddler', text: 'At the beginning of your upkeep, ask a riddle.' },
]

function dueTable(lines = LINES) {
  const manual = vi.fn<(action: ManualAction) => boolean>(() => true)
  const table = renderTable({
    seats: 2,
    options: { viewport: WIDE },
    ui: <CommandsPanel />,
    state: (state) => ({ ...state, dueLines: lines }),
    session: { manual },
  })
  const panel = screen.getByRole('region', { name: 'Commands' })
  return { ...table, manual, panel }
}

describe('To do by hand', () => {
  it('opens the panel with each due line, in order', () => {
    const { panel } = dueTable()
    const group = within(panel).getByRole('group', { name: 'To do by hand' })
    expect(within(panel).getAllByRole('group')[0]).toBe(group)
    expect(
      within(group)
        .getAllByRole('listitem')
        .map((item) => item.textContent),
    ).toEqual([
      'Odd Watcher: “Whenever a creature dies, juggle.”',
      'Odd Watcher: “Whenever a creature dies, juggle.”',
      'Riddler: “At the beginning of your upkeep, ask a riddle.”',
    ])
  })

  it('is not there while nothing is due', () => {
    const { panel } = dueTable([])
    expect(within(panel).queryByRole('group', { name: 'To do by hand' })).toBeNull()
  })

  it('goes on Done, and Done sends nothing', async () => {
    const { panel, user, store, manual } = dueTable()
    await user.click(within(panel).getByRole('button', { name: 'Done' }))
    expect(store.getState().dueLines).toEqual([])
    expect(within(panel).queryByRole('group', { name: 'To do by hand' })).toBeNull()
    expect(manual).not.toHaveBeenCalled()
  })

  it('goes once the store clears it, as a pass of the seat does', () => {
    const { panel, update } = dueTable()
    update(() => ({ dueLines: [] }))
    expect(within(panel).queryByRole('group', { name: 'To do by hand' })).toBeNull()
  })
})
