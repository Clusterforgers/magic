// Goal: after a look at the top the panel lists the cards looked at, each on top, with Top, Bottom and Graveyard and moves up and down by button and by key; the preview and the Apply button say what will be put back, Apply sends exactly one ArrangeTop of the looked-at cards, Cancel and Keep send nothing and keep the cards looked-at, and a seat with no look pending gets the amount form.
import { screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ManualAction } from '@/protocol/generated'
import { buildFixture } from '../../dev/fixture'
import { lookTopMoment } from '../../dev/lookTopScene'
import { renderTable } from '@/test-utils/tableHarness'
import { CommandsPanel } from './CommandsPanel'

const WIDE = { width: 1600, height: 1000 }

function lookTable(looked = true) {
  const manual = vi.fn<(action: ManualAction) => boolean>(() => true)
  const base = buildFixture({ seats: 2 })
  const fixture = looked ? lookTopMoment(base) : base
  const table = renderTable({
    fixture,
    options: { viewport: WIDE },
    ui: <CommandsPanel />,
    session: { manual },
  })
  const view = fixture.view
  const ids = view.players[view.viewer]?.known_top ?? []
  const nameOf = (id: number): string => view.objects.find((o) => o.id === id)?.name ?? ''
  const panel = screen.getByRole('region', { name: 'Commands' })
  const row = within(panel)
    .getAllByRole('button', { name: /^Look at the top/ })
    .find((button) => button.hasAttribute('aria-expanded'))
  if (row === undefined) throw new Error('no Look at the top row')
  return { ...table, manual, panel, row, ids, nameOf }
}

describe('the look-at-the-top row with cards looked at', () => {
  it('says on the row that cards wait, and lists them on top in order when it opens', async () => {
    const { user, panel, row, ids, nameOf } = lookTable()
    expect(row).toHaveTextContent('You looked at the top 3 cards: arrange them.')
    await user.click(row)
    const form = within(panel).getByRole('group', { name: 'Look at the top' })
    const onTop = within(form).getByRole('group', { name: 'Stays on top: 3' })
    const items = within(onTop).getAllByRole('listitem')
    expect(items.map((item) => item.getAttribute('aria-label'))).toEqual(
      ids.map((id, at) => `${nameOf(id)}, top, ${at + 1} of 3`),
    )
    expect(form.querySelector('[data-slot="command-preview"]')).toHaveTextContent('Nothing moves')
    expect(within(form).getByRole('button', { name: 'Keep all 3 on top' })).toBeInTheDocument()
  })

  it('sends one ArrangeTop of exactly the cards looked at as the seat arranged them', async () => {
    const { user, manual, panel, row, ids, nameOf } = lookTable()
    const [first, second, third] = ids
    await user.click(row)
    const form = within(panel).getByRole('group', { name: 'Look at the top' })
    const line = (id: number | undefined) =>
      within(form).getByRole('listitem', { name: new RegExp(`^${nameOf(id ?? 0)},`) })
    await user.click(within(line(second)).getByRole('button', { name: 'Bottom' }))
    await user.click(within(line(third)).getByRole('button', { name: 'Graveyard' }))
    expect(manual).not.toHaveBeenCalled()
    expect(form.querySelector('[data-slot="command-preview"]')).toHaveTextContent(
      /^Scry: .* stays on top; .* goes to the bottom; .* goes to the graveyard\.$/,
    )
    await user.click(within(form).getByRole('button', { name: 'Put 1 on the bottom, 1 in the graveyard' }))
    expect(manual).toHaveBeenCalledTimes(1)
    expect(manual).toHaveBeenCalledWith({
      ArrangeTop: { top: [first], bottom: [second], graveyard: [third] },
    })
  })

  it('reorders the cards on top by the up and down buttons', async () => {
    const { user, manual, panel, row, ids, nameOf } = lookTable()
    await user.click(row)
    const form = within(panel).getByRole('group', { name: 'Look at the top' })
    await user.click(within(form).getByRole('button', { name: `Move ${nameOf(ids[1] ?? 0)} up` }))
    await user.click(within(form).getByRole('button', { name: 'Reorder the 3 on top' }))
    expect(manual).toHaveBeenCalledWith({
      ArrangeTop: { top: [ids[1], ids[0], ids[2]], bottom: [], graveyard: [] },
    })
  })

  it('sends a card by its key and moves it with Alt and the arrows, keeping the focus on the card', async () => {
    const { user, manual, panel, row, ids, nameOf } = lookTable()
    await user.click(row)
    const form = within(panel).getByRole('group', { name: 'Look at the top' })
    const top = within(form).getByRole('button', { name: `Move ${nameOf(ids[0] ?? 0)} down` })
    top.focus()
    await user.keyboard('{Alt>}{ArrowDown}{/Alt}')
    expect(document.activeElement).toHaveAttribute('data-control', 'down')
    await user.keyboard('g')
    const item = within(form).getByRole('listitem', {
      name: new RegExp(`^${nameOf(ids[0] ?? 0)}, graveyard`),
    })
    expect(item).toBeInTheDocument()
    expect(document.activeElement).toHaveAttribute('data-control', 'graveyard')
    await user.click(within(form).getByRole('button', { name: 'Put 1 in the graveyard' }))
    expect(manual).toHaveBeenCalledWith({
      ArrangeTop: { top: [ids[1], ids[0] === undefined ? 0 : ids[2]], bottom: [], graveyard: [ids[0]] },
    })
  })

  it('sends nothing on Cancel or Keep, closes the row and keeps the cards looked-at', async () => {
    const { user, manual, panel, row } = lookTable()
    await user.click(row)
    await user.click(within(panel).getByRole('button', { name: 'Cancel' }))
    expect(row).toHaveAttribute('aria-expanded', 'false')
    expect(row).toHaveTextContent('You looked at the top 3 cards: arrange them.')
    await user.click(row)
    await user.click(within(panel).getByRole('button', { name: 'Keep all 3 on top' }))
    expect(manual).not.toHaveBeenCalled()
    expect(row).toHaveAttribute('aria-expanded', 'false')
  })

  it('closes with Escape', async () => {
    const { user, panel, row } = lookTable()
    await user.click(row)
    within(panel).getAllByRole('button', { name: 'Bottom' })[0]?.focus()
    await user.keyboard('{Escape}')
    expect(row).toHaveAttribute('aria-expanded', 'false')
  })

  it('offers to look at more cards', async () => {
    const { user, manual, panel, row } = lookTable()
    await user.click(row)
    await user.click(within(panel).getByRole('button', { name: 'Look at more' }))
    await user.click(within(panel).getByRole('button', { name: 'Look at the top 1 card' }))
    expect(manual).toHaveBeenCalledWith({ LookTop: { count: 1 } })
  })
})

describe('the look-at-the-top row with no look pending', () => {
  it('asks how many cards and sends the look', async () => {
    const { user, manual, panel, row } = lookTable(false)
    expect(row).toHaveTextContent('Look at the top cards of your library')
    await user.click(row)
    const form = within(panel).getByRole('group', { name: 'Look at the top' })
    await user.click(within(form).getByRole('button', { name: '3' }))
    expect(form.querySelector('[data-slot="command-preview"]')).toHaveTextContent(
      'you see the top 3 cards; the other seats are told how many',
    )
    await user.click(within(form).getByRole('button', { name: 'Look at the top 3 cards' }))
    expect(manual).toHaveBeenCalledWith({ LookTop: { count: 3 } })
  })
})

describe('Reveal the top card', () => {
  it('previews who sees the card and sends the reveal', async () => {
    const { user, manual, panel } = lookTable(false)
    const row = within(panel)
      .getAllByRole('button', { name: /^Reveal the top card/ })
      .find((button) => button.hasAttribute('aria-expanded'))
    if (row === undefined) throw new Error('no row')
    await user.click(row)
    const form = within(panel).getByRole('group', { name: 'Reveal the top card' })
    expect(form.querySelector('[data-slot="command-preview"]')).toHaveTextContent('shown to every seat')
    await user.click(within(form).getByRole('button', { name: 'Reveal the top card' }))
    expect(manual).toHaveBeenCalledWith({ Reveal: { what: { Top: {} } } })
  })
})
