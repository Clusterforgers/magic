// Goal: the Commands panel is the first and default tab of a seat's side column and absent for a viewer that is no seat; a command opens in place, shows the change it will make before anything is sent, sends the exact action on Apply or Enter, says it was sent, and closes on Escape with the focus back on its row; an inert command says why and sends nothing; the Mana row opens on a colour of the seat's commander identity.
import { screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ManualResult } from '@/play/manualLedger'
import type { PlayState } from '@/play/store'
import type { Color, ManaCost, ManualAction } from '@/protocol/generated'
import { card } from '@/test-utils/cards'
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
    ['More life', ['Take 5'], ' life', 'Lose 5 life', { Life: { player: 0, delta: -5 } }],
    ['More life', ['Add 1', 'Add 5'], ' life', 'Gain 6 life', { Life: { player: 0, delta: 6 } }],
    ['Draw', ['2'], 'hand', 'Draw 2 cards', { Draw: { count: 2 } }],
    ['Mill', ['3'], 'go to your graveyard', 'Mill 3 cards', { MoveTop: { count: 3, to: 'Graveyard' } }],
    ['Shuffle', [], 'in a new random order', 'Shuffle your library', { Shuffle: {} }],
  ])('%s with %o previews "%s" and sends on "%s"', async (name, presses, preview, apply, action) => {
    const { user, manual, panel, row } = panelTable()
    await user.click(row(name))
    expect(row(name)).toHaveAttribute('aria-expanded', 'true')
    const form = within(panel).getByRole('group', { name: name === 'More life' ? 'Life' : name })
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
    await user.click(row('More life'))
    const field = screen.getByRole('spinbutton', { name: 'Change' })
    field.focus()
    await user.keyboard('{PageDown}{PageDown}{PageUp}')
    expect(field).toHaveValue(-5)
  })

  it('keeps Apply inert until there is a change, and sends nothing', async () => {
    const { user, manual, row } = panelTable()
    await user.click(row('More life'))
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
    await user.click(row('More life'))
    await user.click(row('Draw'))
    expect(row('More life')).toHaveAttribute('aria-expanded', 'false')
    expect(row('Draw')).toHaveAttribute('aria-expanded', 'true')
  })
})

describe('a row', () => {
  const ROWS = [
    'life',
    'poison',
    'energy',
    'experience',
    'draw',
    'look',
    'mill',
    'exileTop',
    'bottomTop',
    'shuffle',
    'revealTop',
    'token',
    'mana',
    'roll',
  ]

  it('shows its own icon and a chevron, so it reads as a button that opens', () => {
    const { panel } = panelTable()
    const rows = [
      ...panel.querySelectorAll<HTMLElement>('[data-command]:not([data-command^="card:"])'),
    ].filter((row) => row.dataset.command !== 'pick')
    expect(rows.map((row) => row.dataset.command)).toEqual(ROWS)
    const icons = rows.map((row) => row.querySelector('[data-slot="row-icon"]')?.getAttribute('class') ?? '')
    expect(icons.every((icon) => /lucide-/.test(icon))).toBe(true)
    // Within the panel's own groups no two rows share a picture.
    expect(new Set(icons.map((icon) => /lucide-[\w-]+/.exec(icon)?.[0])).size).toBe(ROWS.length)
    for (const row of rows) {
      const chevron = row.querySelector('[data-slot="row-chevron"]')
      expect(chevron).toHaveAttribute('aria-hidden', 'true')
      expect(chevron).toHaveClass('motion-reduce:transition-none')
    }
  })

  it.each(ROWS)('%s turns its chevron down while it is open and back when it closes', async (id) => {
    const { user, panel } = panelTable()
    const row = panel.querySelector<HTMLElement>(`[data-command="${id}"]`)
    const toggle = row?.querySelector<HTMLElement>('button[aria-expanded]')
    const chevron = row?.querySelector('[data-slot="row-chevron"]')
    expect(chevron).not.toHaveClass('rotate-90')
    await user.click(toggle as HTMLElement)
    expect(chevron).toHaveClass('rotate-90')
    await user.click(toggle as HTMLElement)
    expect(chevron).not.toHaveClass('rotate-90')
  })
})

describe('the Life row', () => {
  it('opens under the counter as "More life options", with any amount and its preview', async () => {
    const { user, panel, row } = panelTable()
    const more = row('More life options')
    expect(more).toHaveTextContent('Any amount, with a preview of the change.')
    await user.click(more)
    expect(within(panel).getByRole('group', { name: 'Life' })).toBeInTheDocument()
    expect(within(panel).getByRole('spinbutton', { name: 'Change' })).toBeInTheDocument()
  })

  it('is listed with the reason when the seat cannot act by hand', () => {
    renderTable({
      seats: 2,
      options: { viewport: WIDE },
      ui: <CommandsPanel />,
      state: (state) => ({ ...state, result: 'Draw' }),
    })
    expect(screen.getByRole('button', { name: /^More life options/ })).toHaveTextContent('The game is over.')
  })
})

/** A table where seat 0 owns one commander with these colours and cost. */
function withCommander(colors: Color[], cost: ManaCost | null) {
  return (state: PlayState): PlayState => {
    const object = card({
      id: 900,
      owner: 0,
      controller: 0,
      zone: { Command: 0 },
      name: 'Test Commander',
      colors,
      mana_cost: cost,
      permanent: null,
    })
    if (state.view === null) return state
    return {
      ...state,
      objectsById: { ...state.objectsById, 900: object },
      view: {
        ...state.view,
        players: state.view.players.map((player) =>
          player.seat === 0 ? { ...player, commanders: [{ object: 900, casts: 0, tax: 0 }] } : player,
        ),
      },
    }
  }
}

/** A table of a game with no commanders (the fixture is a Commander table). */
function withoutCommanders(state: PlayState): PlayState {
  if (state.view === null) return state
  return {
    ...state,
    view: { ...state.view, players: state.view.players.map(({ commanders: _gone, ...player }) => player) },
  }
}

describe('the Mana row', () => {
  const ESPER: ManaCost = [{ Generic: 2 }, { Colored: 'White' }, { Colored: 'Blue' }, { Colored: 'Black' }]
  const open = async (state?: (state: PlayState) => PlayState) => {
    const table = renderTable({ seats: 2, options: { viewport: WIDE }, ui: <CommandsPanel />, state })
    const panel = screen.getByRole('region', { name: 'Commands' })
    const row = within(panel)
      .getAllByRole('button', { name: /^Mana/ })
      .find((button) => button.hasAttribute('aria-expanded'))
    if (row === undefined) throw new Error('no Mana row')
    await table.user.click(row)
    return { ...table, panel, select: within(panel).getByRole('combobox', { name: 'Mana' }) }
  }

  it.each<[string, ((state: PlayState) => PlayState) | undefined, string]>([
    ['an Esper commander', withCommander(['White', 'Blue', 'Black'], ESPER), 'White'],
    [
      'a Simic commander',
      withCommander(['Blue', 'Green'], [{ Colored: 'Blue' }, { Colored: 'Green' }]),
      'Blue',
    ],
    ['a red commander', withCommander(['Red'], [{ Colored: 'Red' }]), 'Red'],
    ['a colourless commander', withCommander([], [{ Generic: 7 }]), 'Colorless'],
    ['no commander', withoutCommanders, 'Green'],
  ])('starts on a colour of the identity for %s: %s', async (_what, state, expected) => {
    const { select } = await open(state)
    expect(select).toHaveValue(expected)
  })

  it('starts on the identity again after Apply and keeps what the seat picked until then', async () => {
    const manual = vi.fn<(action: ManualAction) => boolean>(() => true)
    const table = renderTable({
      seats: 2,
      options: { viewport: WIDE },
      ui: <CommandsPanel />,
      state: withCommander(['White', 'Blue', 'Black'], ESPER),
      session: { manual: (action, reply) => (reply?.({ ok: true }), manual(action)) },
    })
    const panel = screen.getByRole('region', { name: 'Commands' })
    const row = within(panel)
      .getAllByRole('button', { name: /^Mana/ })
      .find((button) => button.hasAttribute('aria-expanded'))
    if (row === undefined) throw new Error('no Mana row')
    await table.user.click(row)
    const select = within(panel).getByRole('combobox', { name: 'Mana' })
    await table.user.selectOptions(select, 'Red')
    expect(select).toHaveValue('Red')
    await table.user.click(within(panel).getByRole('button', { name: /^Add/ }))
    expect(manual).toHaveBeenCalledWith({ AddMana: { mana: 'Red', count: 1 } })
    expect(within(panel).getByRole('combobox', { name: 'Mana' })).toHaveValue('White')
  })
})

describe('a command no other command takes back', () => {
  it.each([
    ['Mill', true],
    ['Exile from the top', true],
    ['Top to the bottom', true],
    ['Shuffle', true],
    ['More life', false],
    ['Draw', false],
  ])('%s says it cannot be undone: %s', async (name, lasting) => {
    const { user, panel, row } = panelTable()
    await user.click(row(name))
    const note = panel.querySelector('[data-slot="command-lasting"]')
    if (lasting) expect(note).toHaveTextContent('This cannot be undone')
    else expect(note).toBeNull()
  })
})
