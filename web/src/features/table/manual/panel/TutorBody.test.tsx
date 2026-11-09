/** Goal: once a seat searched its library by hand, the Search for a card row lists the cards under a search box that takes focus, narrows by every word of name or type line, counts the cards in view, says when none matches, goes on to the cards with Arrow Down and along them with the arrow keys, clears with Escape before the row closes, and keeps the card chosen while the search hides it. */
import { screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ManualAction } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from '../../dev/fixture'
import { LOOKED_MOMENTS } from '../../dev/lookedPrompts'
import type { Fixture } from '../../dev/fixture'
import { CommandsPanel } from './CommandsPanel'

/** The fixture with the viewer's whole library searched by hand: `known_top` and the log lines of the search. */
function searchedMoment(fixture: Fixture): Fixture {
  const withLibrary = LOOKED_MOMENTS.scry?.(fixture) ?? fixture
  const { view } = withLibrary
  const cards = view.objects
    .filter((o) => typeof o.zone === 'object' && 'Library' in o.zone && o.zone.Library === view.viewer)
    .map((o) => o.id)
  const seq = (withLibrary.log.at(-1)?.seq ?? -1) + 1
  return {
    ...withLibrary,
    view: {
      ...view,
      players: view.players.map((player) =>
        player.seat === view.viewer ? { ...player, known_top: cards } : player,
      ),
    },
    log: [
      ...withLibrary.log,
      { seq, event: { Public: { ManualAction: { player: view.viewer, action: { Search: {} } } } } },
      { seq: seq + 1, event: { Public: { LibraryLooked: { player: view.viewer, cards } } } },
    ],
  }
}

async function openSearch() {
  const manual = vi.fn<(action: ManualAction) => boolean>(() => true)
  const fixture = searchedMoment(buildFixture({ seats: 2 }))
  const table = renderTable({
    fixture,
    options: { viewport: { width: 1600, height: 1000 } },
    ui: <CommandsPanel />,
    session: { manual },
  })
  const panel = screen.getByRole('region', { name: 'Commands' })
  const row = must(
    within(panel)
      .getAllByRole('button', { name: /^Search for a card/ })
      .find((button) => button.hasAttribute('aria-expanded')),
  )
  await table.user.click(row)
  const form = within(panel).getByRole('group', { name: 'Search your library' })
  const box = (): HTMLElement => within(form).getByRole('searchbox', { name: 'Filter cards' })
  const cards = (): HTMLElement[] => [...form.querySelectorAll<HTMLElement>('button[data-card]')]
  const names = (): string[] => cards().map((card) => card.textContent ?? '')
  const count = (): string => must(form.querySelector('[data-slot="filter-count"]')).textContent ?? ''
  const library = fixture.view.objects.filter(
    (o) => typeof o.zone === 'object' && 'Library' in o.zone && o.zone.Library === fixture.view.viewer,
  )
  return { ...table, manual, panel, form, box, cards, names, count, library }
}

describe('the search grid of a search by hand', () => {
  it('opens with the search box focused and every card counted', async () => {
    const { box, count, cards, library } = await openSearch()
    expect(box()).toHaveFocus()
    expect(cards()).toHaveLength(library.length)
    expect(count()).toBe(`${library.length} of ${library.length}`)
  })

  it('narrows by every word of the name or type line, ignoring case', async () => {
    const { user, box, names, count, library } = await openSearch()
    const name = must(library[0]).name
    await user.type(box(), name.toUpperCase())
    expect(names().every((shown) => shown.toLowerCase().includes(name.toLowerCase()))).toBe(true)
    expect(names()).toContain(name)
    expect(count()).toBe(`${names().length} of ${library.length}`)
  })

  it('says when no card matches, and Escape clears the words without closing the row', async () => {
    const { user, box, form, cards, library } = await openSearch()
    await user.type(box(), 'zzzz')
    expect(cards()).toHaveLength(0)
    expect(must(form.querySelector('[data-slot="no-match"]')).textContent).toContain('zzzz')
    await user.keyboard('{Escape}')
    expect(box()).toHaveValue('')
    expect(form).toBeInTheDocument()
    expect(cards()).toHaveLength(library.length)
  })

  it('goes on to the cards with Arrow Down and along them with the arrow keys', async () => {
    const { user, cards } = await openSearch()
    await user.keyboard('{ArrowDown}')
    expect(cards()[0]).toHaveFocus()
    await user.keyboard('{ArrowRight}')
    expect(cards()[1]).toHaveFocus()
    await user.keyboard('{Enter}')
    expect(cards()[1]).toHaveAttribute('aria-pressed', 'true')
    // The chosen card wears the chosen colour and a check; the others neither.
    expect(cards().map((card) => card.querySelector('[data-slot="chosen-mark"]') !== null)).toEqual(
      cards().map((_, index) => index === 1),
    )
    expect(cards()[1]).toHaveClass('border-selected')
  })

  it('keeps the card chosen while the search hides it, and Apply takes it', async () => {
    const { user, box, cards, count, form, manual, library } = await openSearch()
    const card = must(cards()[0])
    const id = Number(card.getAttribute('data-card'))
    await user.click(card)
    await user.type(box(), 'zzzz')
    expect(count()).toBe(`0 of ${library.length} · 1 chosen out of view`)
    await user.click(must(form.querySelector<HTMLElement>('[data-slot="command-apply"]')))
    expect(manual.mock.calls[0]?.[0]).toEqual({ Tutor: { object: id } })
  })
})
