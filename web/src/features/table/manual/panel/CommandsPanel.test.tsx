// Goal: the Commands panel is the first and default tab of a seat's side column and absent for a viewer that is no seat; a command opens in place, shows the change it will make before anything is sent, sends the exact action on Apply or Enter, says it was sent, and closes on Escape with the focus back on its row; an inert command says why and sends nothing.
import { screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ManualResult } from '@/play/manualLedger'
import type { ManualAction } from '@/protocol/generated'
import { renderTable } from '@/test-utils/tableHarness'
import { CommandsPanel } from './CommandsPanel'

const WIDE = { width: 1600, height: 1000 }

function panelTable(
  manual = vi.fn<(action: ManualAction) => boolean>(() => true),
  answer: ManualResult | 'silence' = { ok: true },
) {
  const table = renderTable({
    seats: 2,
    options: { viewport: WIDE },
    ui: <CommandsPanel />,
    session: {
      manual: (action, reply) => {
        const taken = manual(action)
        if (taken && answer !== 'silence') reply?.(answer)
        return taken
      },
    },
  })
  const panel = screen.getByRole('region', { name: 'Commands' })
  const row = (name: string): HTMLElement => {
    const found = within(panel)
      .getAllByRole('button', { name: new RegExp(`^${name}`) })
      .find((button) => button.hasAttribute('aria-expanded'))
    if (found === undefined) throw new Error(`no row ${name}`)
    return found
  }
  return { ...table, manual, panel, row }
}

describe('the Commands tab', () => {
  it('is the first tab of a seat and on show when the table opens', () => {
    renderTable({ seats: 2, options: { viewport: WIDE } })
    const side = screen.getByRole('complementary', { name: 'Commands, log, chat and stack' })
    const tabs = within(side).getAllByRole('tab')
    expect(tabs[0]).toHaveTextContent('Commands')
    expect(tabs[0]).toHaveAttribute('aria-selected', 'true')
    expect(within(side).getByRole('region', { name: 'Commands' })).toBeInTheDocument()
  })

  it('is not there for a viewer that is no seat', () => {
    renderTable({ seats: 2, options: { viewport: WIDE }, state: (state) => ({ ...state, seat: null }) })
    expect(screen.queryByRole('tab', { name: 'Commands' })).toBeNull()
    expect(screen.queryByRole('region', { name: 'Commands' })).toBeNull()
  })
})

describe('a command', () => {
  it.each<[string, string[], string, string, ManualAction]>([
    ['Life', ['Take 5'], ' life', 'Lose 5 life', { Life: { player: 0, delta: -5 } }],
    ['Life', ['Add 1', 'Add 5'], ' life', 'Gain 6 life', { Life: { player: 0, delta: 6 } }],
    ['Draw', ['2'], 'hand', 'Draw 2 cards', { Draw: { count: 2 } }],
    ['Mill', ['3'], 'go to your graveyard', 'Mill 3 cards', { MoveTop: { count: 3, to: 'Graveyard' } }],
    ['Shuffle', [], 'in a new random order', 'Shuffle your library', { Shuffle: {} }],
  ])('%s with %o previews "%s" and sends on "%s"', async (name, presses, preview, apply, action) => {
    const { user, manual, panel, row } = panelTable()
    await user.click(row(name))
    expect(row(name)).toHaveAttribute('aria-expanded', 'true')
    const form = within(panel).getByRole('group', { name })
    for (const press of presses) await user.click(within(form).getByRole('button', { name: press }))
    expect(manual).not.toHaveBeenCalled()
    expect(form.querySelector('[data-slot="command-preview"]')).toHaveTextContent(preview)
    await user.click(within(form).getByRole('button', { name: apply }))
    expect(manual).toHaveBeenCalledWith(action)
    expect(within(form).getByRole('status')).toHaveTextContent(`Done: ${apply}.`)
  })

  it('applies with Enter in the amount field and closes with Escape', async () => {
    const { user, manual, row } = panelTable()
    await user.click(row('Draw'))
    const field = screen.getByRole('spinbutton', { name: 'How many' })
    await user.clear(field)
    await user.type(field, '3{Enter}')
    expect(manual).toHaveBeenCalledWith({ Draw: { count: 3 } })
    await user.keyboard('{Escape}')
    expect(row('Draw')).toHaveAttribute('aria-expanded', 'false')
    expect(row('Draw')).toHaveFocus()
  })

  it('steps a change by five with Page Up and Page Down', async () => {
    const { user, row } = panelTable()
    await user.click(row('Life'))
    const field = screen.getByRole('spinbutton', { name: 'Change' })
    field.focus()
    await user.keyboard('{PageDown}{PageDown}{PageUp}')
    expect(field).toHaveValue(-5)
  })

  it('keeps Apply inert until there is a change, and sends nothing', async () => {
    const { user, manual, row } = panelTable()
    await user.click(row('Life'))
    const apply = screen.getByRole('button', { name: 'Apply' })
    expect(apply).toHaveAttribute('aria-disabled', 'true')
    await user.click(apply)
    expect(manual).not.toHaveBeenCalled()
    expect(screen.getByText('Choose an amount with the buttons or the field.')).toBeInTheDocument()
  })

  it('says when the command could not be sent', async () => {
    const { user, row } = panelTable(vi.fn(() => false))
    await user.click(row('Shuffle'))
    await user.click(screen.getByRole('button', { name: 'Shuffle your library' }))
    expect(screen.getByRole('status')).toHaveTextContent('Not sent: the connection is not open.')
  })

  it('shows the reason of a refusal in its own row, keeps the choices and leaves the table notice alone', async () => {
    const { user, row, store } = panelTable(undefined, { ok: false, reason: 'you are not in the game' })
    await user.click(row('Draw'))
    await user.click(screen.getByRole('button', { name: '3' }))
    await user.click(screen.getByRole('button', { name: 'Draw 3 cards' }))
    const line = screen.getByRole('status')
    expect(line).toHaveTextContent('Refused: you are not in the game.')
    expect(line).toHaveAttribute('data-state', 'refused')
    expect(screen.getByRole('spinbutton', { name: 'How many' })).toHaveValue(3)
    expect(store.getState().notice).toBeNull()
  })

  it('says Sending until the server answers, and holds a second Apply back', async () => {
    const { user, row, manual } = panelTable(undefined, 'silence')
    await user.click(row('Shuffle'))
    await user.click(screen.getByRole('button', { name: 'Shuffle your library' }))
    expect(screen.getByRole('status')).toHaveTextContent('Sending: Shuffle your library…')
    await user.click(screen.getByRole('button', { name: 'Shuffle your library' }))
    expect(manual).toHaveBeenCalledTimes(1)
  })

  it('opens one row at a time', async () => {
    const { user, row } = panelTable()
    await user.click(row('Life'))
    await user.click(row('Draw'))
    expect(row('Life')).toHaveAttribute('aria-expanded', 'false')
    expect(row('Draw')).toHaveAttribute('aria-expanded', 'true')
  })
})

describe('a command no other command takes back', () => {
  it.each([
    ['Mill', true],
    ['Exile from the top', true],
    ['Top to the bottom', true],
    ['Shuffle', true],
    ['Life', false],
    ['Draw', false],
  ])('%s says it cannot be undone: %s', async (name, lasting) => {
    const { user, panel, row } = panelTable()
    await user.click(row(name))
    const note = panel.querySelector('[data-slot="command-lasting"]')
    if (lasting) expect(note).toHaveTextContent('This cannot be undone')
    else expect(note).toBeNull()
  })
})
