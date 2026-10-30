// Goal: "Pick a card" arms the table with a banner and Escape to cancel, a press on any card by mouse, keyboard or finger chooses it without acting on it, the Card group then lists the card's commands with the preview and Apply pattern, the card menu's "In the panel" fills it too, a refusal shows in the row of the command, and a card that leaves the table is forgotten.
import { act, fireEvent, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ManualResult } from '@/play/manualLedger'
import type { ManualAction } from '@/protocol/generated'
import { indexById } from '@/play/view'
import { clone, must } from '@/test-utils/batches'
import { card } from '@/test-utils/cards'
import { renderTable } from '@/test-utils/tableHarness'
import { Card } from '../../Card'
import { buildFixture } from '../../dev/fixture'
import { ManualMenuProvider } from '../ManualMenu'
import { CommandsPanel } from './CommandsPanel'

const MINE = 900
const THEIRS = 901
const WIDE = { width: 1600, height: 1000 }

function renderPick(answer: ManualResult | 'silence' = { ok: true }) {
  const manual = vi.fn<(action: ManualAction) => boolean>(() => true)
  const table = renderTable({
    fixture: buildFixture({ seats: 2, permanents: 4, hand: 2 }),
    options: { viewport: WIDE },
    ui: (
      <ManualMenuProvider>
        <CommandsPanel />
        <Card id={MINE} />
        <Card id={THEIRS} />
      </ManualMenuProvider>
    ),
    session: {
      manual: (action, reply) => {
        manual(action)
        if (answer !== 'silence') reply?.(answer)
        return true
      },
    },
    state: (first) => {
      const view = clone(must(first.view))
      view.objects = [
        ...view.objects,
        card({ id: MINE, name: 'Grizzly Bears' }),
        card({ id: THEIRS, name: 'Wolf', owner: 1, controller: 1 }),
      ]
      return { ...first, view, objectsById: indexById(view.objects) }
    },
  })
  return { ...table, manual }
}

const cardButton = (id: number): HTMLElement =>
  document.querySelector<HTMLElement>(`button[data-object-id="${id}"]`) as HTMLElement
const group = (): HTMLElement => document.querySelector<HTMLElement>('[data-group="card"]') as HTMLElement
const pickButton = (): HTMLElement =>
  document.querySelector<HTMLElement>('[data-command="pick"]') as HTMLElement
const commandRow = (name: RegExp): HTMLElement =>
  within(group())
    .getAllByRole('button', { name })
    .find((button) => button.hasAttribute('aria-expanded')) as HTMLElement
const picked = (): string | null => group().querySelector('[data-slot="card-picked"]')?.textContent ?? null

describe('before a card is picked', () => {
  it('says what to do and offers no card command', () => {
    renderPick()
    expect(within(group()).getByText('Pick a card on the table to act on it.')).toBeInTheDocument()
    expect(pickButton()).toHaveAccessibleName(/^Pick a card/)
    expect(group().querySelectorAll('[data-card-command]')).toHaveLength(0)
  })

  it('arms with a banner "Pick a card for its commands", marks the table and cancels on Escape', async () => {
    const { user } = renderPick()
    await user.click(pickButton())
    expect(document.querySelector('[data-slot="pick-banner"]')).toHaveTextContent(
      'Pick a card for its commands. Escape cancels.',
    )
    expect(pickButton()).toHaveAttribute('aria-pressed', 'true')
    expect(document.documentElement).toHaveAttribute('data-picking')
    await user.keyboard('{Escape}')
    expect(document.querySelector('[data-slot="pick-banner"]')).toBeNull()
    expect(document.documentElement).not.toHaveAttribute('data-picking')
    expect(pickButton()).toHaveAttribute('aria-pressed', 'false')
  })

  it('cancels with the banner button, and with the pick button pressed again', async () => {
    const { user } = renderPick()
    await user.click(pickButton())
    await user.click(screen.getByRole('button', { name: 'Cancel' }))
    expect(document.querySelector('[data-slot="pick-banner"]')).toBeNull()
    await user.click(pickButton())
    await user.click(pickButton())
    expect(document.querySelector('[data-slot="pick-banner"]')).toBeNull()
  })
})

describe('choosing the card', () => {
  it('takes a click, and does not act on the card or pin its preview', async () => {
    const { user } = renderPick()
    await user.click(pickButton())
    await user.click(cardButton(MINE))
    expect(picked()).toContain('Grizzly Bears')
    expect(screen.queryByTestId('card-preview')).toBeNull()
    expect(document.querySelector('[data-slot="pick-banner"]')).toBeNull()
  })

  it.each([
    ['Enter', '{Enter}'],
    ['Space', ' '],
  ])('takes %s on the focused card', async (_name, keys) => {
    const { user } = renderPick()
    await user.click(pickButton())
    cardButton(THEIRS).focus()
    await user.keyboard(keys)
    expect(picked()).toContain('Wolf')
  })

  it('takes a tap of a finger, and a long press does not open the card menu while it waits', () => {
    vi.useFakeTimers()
    renderPick()
    act(() => pickButton().click())
    fireEvent.pointerDown(cardButton(MINE), { pointerType: 'touch', clientX: 10, clientY: 10 })
    act(() => {
      vi.advanceTimersByTime(800)
    })
    expect(screen.queryByRole('menu')).toBeNull()
    fireEvent.pointerUp(cardButton(MINE), { pointerType: 'touch' })
    fireEvent.click(cardButton(MINE))
    expect(picked()).toContain('Grizzly Bears')
    vi.useRealTimers()
  })

  it('takes a right click too, without opening the menu', async () => {
    const { user } = renderPick()
    await user.click(pickButton())
    fireEvent.contextMenu(cardButton(MINE))
    expect(picked()).toContain('Grizzly Bears')
    expect(screen.queryByRole('menu')).toBeNull()
  })

  it('can be redone: Pick another card, and Escape returns to the card it had', async () => {
    const { user } = renderPick()
    await user.click(pickButton())
    await user.click(cardButton(MINE))
    expect(pickButton()).toHaveAccessibleName(/^Pick another card/)
    await user.click(pickButton())
    await user.keyboard('{Escape}')
    expect(picked()).toContain('Grizzly Bears')
    await user.click(pickButton())
    await user.click(cardButton(THEIRS))
    expect(picked()).toContain('Wolf')
  })

  it('moves the focus to the card picked, so the keyboard goes on with its commands', async () => {
    const { user } = renderPick()
    await user.click(pickButton())
    await user.click(cardButton(MINE))
    expect(group().querySelector('[data-slot="card-picked"]')).toHaveFocus()
  })

  it('clears the pick with its button, and forgets a card that left the table', async () => {
    const { user, update } = renderPick()
    await user.click(pickButton())
    await user.click(cardButton(MINE))
    await user.click(screen.getByRole('button', { name: 'Clear the picked card' }))
    expect(picked()).toBeNull()
    await user.click(pickButton())
    await user.click(cardButton(THEIRS))
    update((state) => {
      const view = { ...must(state.view), objects: must(state.view).objects.filter((o) => o.id !== THEIRS) }
      return { view, objectsById: indexById(view.objects) }
    })
    expect(picked()).toBeNull()
    expect(within(group()).getByText('Pick a card on the table to act on it.')).toBeInTheDocument()
  })
})

describe('the commands of the card picked', () => {
  async function pick(id = MINE, answer?: ManualResult | 'silence') {
    const harness = renderPick(answer)
    await harness.user.click(pickButton())
    await harness.user.click(cardButton(id))
    return harness
  }

  it('lists what its menu lists, each a row with its effect', async () => {
    await pick()
    const rows = [...group().querySelectorAll('[data-card-command]')].map((row) =>
      row.getAttribute('data-command'),
    )
    expect(rows).toEqual(['card:move', 'card:counters', 'card:tap', 'card:damage', 'card:pump'])
    expect(commandRow(/^Tap/)).toHaveTextContent('Turn the permanent sideways.')
  })

  it('gives each command row its icon and a chevron that turns down while it is open', async () => {
    const { user } = await pick()
    const rows = [...group().querySelectorAll<HTMLElement>('[data-card-command]')]
    expect(rows).toHaveLength(5)
    for (const row of rows) {
      expect(row.querySelector('[data-slot="row-icon"]')).not.toBeNull()
      expect(row.querySelector('[data-slot="row-chevron"]')).not.toHaveClass('rotate-90')
    }
    await user.click(commandRow(/^Tap/))
    const open = group().querySelector('[data-command="card:tap"] [data-slot="row-chevron"]')
    expect(open).toHaveClass('rotate-90')
    expect(group().querySelector('[data-command="card:move"] [data-slot="row-chevron"]')).not.toHaveClass(
      'rotate-90',
    )
  })

  it("explains an opponent's card: the move row is inert with the reason", async () => {
    await pick(THEIRS)
    expect(commandRow(/^Move to/)).toHaveTextContent(/Only .* who owns or controls it/)
  })

  it('previews a tap, sends nothing before Apply, then sends it and says it was done', async () => {
    const { user, manual } = await pick()
    await user.click(commandRow(/^Tap/))
    const form = within(group()).getByRole('group', { name: 'Tap' })
    expect(form.querySelector('[data-slot="command-preview"]')).toHaveTextContent(
      'Grizzly Bears: untapped → tapped',
    )
    expect(manual).not.toHaveBeenCalled()
    await user.click(within(form).getByRole('button', { name: 'Tap Grizzly Bears' }))
    expect(manual).toHaveBeenCalledWith({ Tap: { object: MINE } })
    expect(within(form).getByRole('status')).toHaveTextContent('Done: Tap Grizzly Bears.')
  })

  it('adds counters from the amount buttons and Enter', async () => {
    const { user, manual } = await pick()
    await user.click(commandRow(/^Counters/))
    const form = within(group()).getByRole('group', { name: 'Counters' })
    await user.click(within(form).getByRole('button', { name: 'Change: add 1' }))
    await user.click(within(form).getByRole('button', { name: 'Change: add 1' }))
    expect(form.querySelector('[data-slot="command-preview"]')).toHaveTextContent('+1/+1 0 → 2')
    await user.type(within(form).getByRole('spinbutton', { name: 'Change' }), '{Enter}')
    expect(manual).toHaveBeenCalledWith({ Counters: { object: MINE, counter: '+1/+1', delta: 2 } })
  })

  it('chooses the place of a move and the target of damage', async () => {
    const { user, manual } = await pick()
    await user.click(commandRow(/^Move to/))
    await user.selectOptions(within(group()).getByLabelText('Move to'), 'Exile')
    await user.click(within(group()).getByRole('button', { name: 'Move to exile' }))
    expect(manual).toHaveBeenLastCalledWith({ Move: { object: MINE, to: 'Exile' } })
    await user.click(commandRow(/^Deal damage/))
    expect(within(group()).getByRole('button', { name: 'Apply' })).toHaveAttribute('aria-disabled', 'true')
    await user.selectOptions(within(group()).getByLabelText('Deal damage to'), 'Bo')
    await user.click(within(group()).getByRole('button', { name: '3' }))
    await user.click(within(group()).getByRole('button', { name: 'Deal 3 damage' }))
    expect(manual).toHaveBeenLastCalledWith({
      Damage: { source: MINE, target: { Player: 1 }, amount: 3 },
    })
  })

  it('shows a refusal in the row of the command, not in the table notice', async () => {
    const { user, store } = await pick(MINE, { ok: false, reason: 'Grizzly Bears is already tapped' })
    await user.click(commandRow(/^Tap/))
    await user.click(within(group()).getByRole('button', { name: 'Tap Grizzly Bears' }))
    const line = within(group()).getByRole('status')
    expect(line).toHaveTextContent('Refused: Grizzly Bears is already tapped.')
    expect(line).toHaveAttribute('data-state', 'refused')
    expect(store.getState().notice).toBeNull()
  })

  it('opens one row at a time across the panel, closes on Escape with the focus on its row', async () => {
    const { user } = await pick()
    await user.click(commandRow(/^Tap/))
    await user.click(screen.getByRole('button', { name: /^More life/ }))
    expect(commandRow(/^Tap/)).toHaveAttribute('aria-expanded', 'false')
    await user.click(commandRow(/^Tap/))
    within(group()).getByRole('button', { name: 'Tap Grizzly Bears' }).focus()
    await user.keyboard('{Escape}')
    expect(commandRow(/^Tap/)).toHaveAttribute('aria-expanded', 'false')
    expect(commandRow(/^Tap/)).toHaveFocus()
  })
})

describe('on a window with a rail', () => {
  it('steps the panel aside while a card is waited for and brings it back with the card', async () => {
    const { user } = renderTable({ seats: 2, options: { viewport: { width: 1100, height: 800 } } })
    await user.click(screen.getByRole('tab', { name: 'Commands' }))
    const rail = (): Element | null => document.querySelector('[data-slot="rail-panel"]')
    expect(rail()).not.toBeNull()
    await user.click(pickButton())
    expect(rail()).toBeNull()
    expect(document.querySelector('[data-slot="pick-banner"]')).not.toBeNull()
    await user.click(document.querySelector('button[data-object-id]') as HTMLElement)
    expect(rail()).not.toBeNull()
    expect(group().querySelector('[data-slot="card-picked"]')).not.toBeNull()
  })
})

describe('the card menu in the panel', () => {
  it('fills the Card group from "In the panel"', async () => {
    const { user } = renderPick()
    cardButton(MINE).focus()
    await user.keyboard('m')
    await user.click(screen.getByRole('menuitem', { name: /^In the panel/ }))
    expect(picked()).toContain('Grizzly Bears')
    expect(commandRow(/^Tap/)).toBeInTheDocument()
  })
})
