// Goal: the Roll row of the Commands panel opens on a d20, previews what it will roll, sends the coin, the d6, the d20 or a die of the size typed on Apply or Enter only, refuses a size outside 2 to 100 with the reason and sends nothing, and is inert once the game is over.
import { screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ManualAction } from '@/protocol/generated'
import { renderTable } from '@/test-utils/tableHarness'
import type { TableSetup } from '@/test-utils/tableHarness'
import { CommandsPanel } from './CommandsPanel'

const ROW = 'Roll a die / Flip a coin'

function panelTable(state?: NonNullable<TableSetup['state']>) {
  const manual = vi.fn<(action: ManualAction) => boolean>(() => true)
  const table = renderTable({
    seats: 2,
    options: { viewport: { width: 1600, height: 1000 } },
    ui: <CommandsPanel />,
    session: {
      manual: (action, reply) => {
        const taken = manual(action)
        if (taken) reply?.({ ok: true })
        return taken
      },
    },
    ...(state === undefined ? {} : { state }),
  })
  const panel = screen.getByRole('region', { name: 'Commands' })
  const row = within(panel)
    .getAllByRole('button', { name: new RegExp(`^${ROW}`) })
    .find((button) => button.hasAttribute('aria-expanded'))
  if (row === undefined) throw new Error('no roll row')
  return { ...table, manual, panel, row }
}

const previewOf = (form: HTMLElement): string | null =>
  form.querySelector('[data-slot="command-preview"]')?.textContent ?? null

describe('the Roll row', () => {
  it('sits in Make and opens on a d20 with its preview', async () => {
    const { user, panel, row } = panelTable()
    expect(within(panel).getByRole('group', { name: 'Make' })).toContainElement(row)
    await user.click(row)
    const form = within(panel).getByRole('group', { name: ROW })
    expect(within(form).getByRole('button', { name: 'd20' })).toHaveAttribute('aria-pressed', 'true')
    expect(previewOf(form)).toBe('Rolls a d20; everyone sees the result')
    expect(within(form).getByRole('button', { name: 'Roll a d20' })).toBeInTheDocument()
  })

  it.each<[string, string, string, ManualAction]>([
    ['Coin', 'Flips a coin; everyone sees the result', 'Flip a coin', { Roll: { kind: { Coin: {} } } }],
    ['d6', 'Rolls a d6; everyone sees the result', 'Roll a d6', { Roll: { kind: { Die: { sides: 6 } } } }],
    ['d20', 'Rolls a d20; everyone sees the result', 'Roll a d20', { Roll: { kind: { Die: { sides: 20 } } } }],
  ])('%s previews "%s" and sends only on "%s"', async (press, preview, apply, action) => {
    const { user, manual, panel, row } = panelTable()
    await user.click(row)
    const form = within(panel).getByRole('group', { name: ROW })
    await user.click(within(form).getByRole('button', { name: press }))
    expect(previewOf(form)).toBe(preview)
    expect(manual).not.toHaveBeenCalled()
    await user.click(within(form).getByRole('button', { name: apply }))
    expect(manual).toHaveBeenCalledExactlyOnceWith(action)
    expect(within(form).getByRole('status')).toHaveTextContent(`Done: ${apply}.`)
  })

  it('takes a die of any size from the field and applies with Enter', async () => {
    const { user, manual, panel, row } = panelTable()
    await user.click(row)
    const form = within(panel).getByRole('group', { name: ROW })
    expect(within(form).queryByRole('spinbutton')).toBeNull()
    await user.click(within(form).getByRole('button', { name: 'd N' }))
    const faces = within(form).getByRole('spinbutton', { name: 'Faces' })
    await user.clear(faces)
    await user.type(faces, '12')
    expect(previewOf(form)).toBe('Rolls a d12; everyone sees the result')
    await user.type(faces, '{Enter}')
    expect(manual).toHaveBeenCalledExactlyOnceWith({ Roll: { kind: { Die: { sides: 12 } } } })
  })

  it.each(['1', '101', ''])('refuses %j faces with the range and sends nothing', async (typed) => {
    const { user, manual, panel, row } = panelTable()
    await user.click(row)
    const form = within(panel).getByRole('group', { name: ROW })
    await user.click(within(form).getByRole('button', { name: 'd N' }))
    const faces = within(form).getByRole('spinbutton', { name: 'Faces' })
    await user.clear(faces)
    if (typed !== '') await user.type(faces, typed)
    expect(previewOf(form)).toBe('Choose a die of 2 to 100 faces.')
    await user.click(within(form).getByRole('button', { name: 'Apply' }))
    await user.type(faces, '{Enter}')
    expect(manual).not.toHaveBeenCalled()
  })

  it('is inert with the reason once the game is over', () => {
    const { row } = panelTable((state) => ({ ...state, result: 'Draw' }))
    expect(row).toHaveTextContent('The game is over')
  })
})
