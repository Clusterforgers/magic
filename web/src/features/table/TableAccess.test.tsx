// Goal: the table is usable from the keyboard and by a screen reader: each row of cards is one tab stop moved with the arrow keys, every region and control has a name, the turn badge and the read-only decision line say where the game is.
import { act, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { indexById } from '@/play/view'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('the keyboard', () => {
  it('makes each row of cards one tab stop and moves along it with the arrow keys, Home and End', async () => {
    const { user } = renderTable({ seats: 2 })
    const row = screen.getByRole('list', { name: "Bo's creatures" })
    const cards = within(row).getAllByRole('button')
    expect(cards).toHaveLength(14)
    expect(cards.filter((card) => card.getAttribute('tabindex') === '0')).toEqual([cards[0]])
    act(() => must(cards[0]).focus())
    await user.keyboard('{ArrowRight}{ArrowRight}')
    expect(cards[2]).toHaveFocus()
    expect(cards.filter((card) => card.getAttribute('tabindex') === '0')).toEqual([cards[2]])
    await user.keyboard('{End}')
    expect(cards.at(-1)).toHaveFocus()
    await user.keyboard('{ArrowRight}')
    expect(cards.at(-1)).toHaveFocus()
    await user.keyboard('{Home}')
    expect(cards[0]).toHaveFocus()
    await user.keyboard('{ArrowLeft}')
    expect(cards[0]).toHaveFocus()
  })

  it('keeps the whole table down to a few dozen tab stops, not one per card', () => {
    const { container } = renderTable({ seats: 4 })
    const cards = container.querySelectorAll('button[data-object-id]').length
    const stops = container.querySelectorAll('button[data-object-id]:not([tabindex="-1"])').length
    expect(cards).toBeGreaterThan(150)
    expect(stops).toBeLessThan(30)
  })

  it("keeps a row's tab stop on the card that was last focused when the row changes around it", async () => {
    const { user, update, store } = renderTable({ seats: 2 })
    const row = () => screen.getByRole('list', { name: "Bo's creatures" })
    const cards = within(row()).getAllByRole('button')
    act(() => must(cards[0]).focus())
    await user.keyboard('{ArrowRight}')
    const chosen = must(cards[1]).getAttribute('data-object-id')
    // The creature in front of it leaves the battlefield.
    const leaving = Number(must(cards[0]).getAttribute('data-object-id'))
    update((state) => {
      const view = clone(must(state.view))
      view.objects = view.objects.filter((object) => object.id !== leaving)
      view.battlefield = {
        count: view.battlefield.count - 1,
        cards: must(view.battlefield.cards).filter((id) => id !== leaving),
      }
      return { view, objectsById: indexById(view.objects) }
    })
    expect(store.getState().objectsById[leaving]).toBeUndefined()
    const tabbable = within(row())
      .getAllByRole('button')
      .filter((card) => card.getAttribute('tabindex') === '0')
    expect(tabbable.map((card) => card.getAttribute('data-object-id'))).toEqual([chosen])
  })
})

describe('regions and the rest of the page', () => {
  it('names every region and control a keyboard or screen-reader user meets', () => {
    renderTable({ seats: 3 })
    expect(screen.getByRole('region', { name: 'Game table' })).toBeInTheDocument()
    expect(screen.getByRole('heading', { level: 1, name: 'Game table' })).toBeInTheDocument()
    for (const name of [
      'Turn and priority',
      'Your hand',
      'Pass controls',
      'Bo',
      'Cy',
      'Alice (you)',
      "Bo's battlefield",
      "Alice's battlefield",
    ]) {
      expect(screen.getByRole('region', { name }), name).toBeInTheDocument()
    }
    expect(screen.getByRole('complementary', { name: 'Commands, log, chat and stack' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Table menu' })).toBeInTheDocument()
    for (const control of screen.getAllByRole('button')) expect(control).toHaveAccessibleName()
    for (const list of screen.getAllByRole('list')) {
      if (list.closest('aside') === null) expect(list).toHaveAccessibleName()
    }
  })

  it('shows a plain waiting note before there is a view', () => {
    renderTable({ seats: 2, state: (state) => ({ ...state, view: null }) })
    expect(screen.getByRole('region', { name: 'Game table' })).toHaveTextContent('Waiting for the table…')
    expect(document.querySelector('[data-seat]')).toBeNull()
  })

  it('shows a plain waiting note while this seat is not known', () => {
    renderTable({ seats: 2, state: (state) => ({ ...state, seat: null }) })
    expect(screen.getByRole('region', { name: 'Game table' })).toHaveTextContent('Waiting for the table…')
  })
})

describe('turn badge and decision line', () => {
  it('prints the turn in the top bar, and the priority holder and the decision this seat faces in the strip', () => {
    renderTable({ seats: 4 })
    expect(document.querySelector('[data-slot="banner"]')).toHaveTextContent(
      "Bo's turn · 5 · Declare blockers",
    )
    const strip = screen.getByRole('region', { name: 'Turn and priority' })
    expect(strip).toHaveTextContent('Alice has priority · 1 passed')
    expect(strip).toHaveTextContent('Your decision: Priority')
  })

  it('says who the game waits for once this seat has answered', () => {
    const { update } = renderTable({ seats: 4 })
    update((state) => {
      const view = clone(must(state.view))
      view.priority = { holder: 2, passed: [] }
      return { view, pending: null }
    })
    const strip = screen.getByRole('region', { name: 'Turn and priority' })
    expect(strip).toHaveTextContent('Cy has priority')
    expect(strip).toHaveTextContent('Waiting for Cy')
    expect(strip).not.toHaveTextContent('Your decision')
  })

  it('is read-only while another seat decides: nothing in the strip answers, only the spells on the stack show', () => {
    renderTable({ seats: 4, state: (state) => ({ ...state, pending: null }) })
    const buttons = within(screen.getByRole('region', { name: 'Turn and priority' })).queryAllByRole('button')
    expect(buttons.length).toBeGreaterThan(0)
    for (const button of buttons) expect(button.closest('[data-slot="stack-chips"]')).not.toBeNull()
    expect(document.querySelector('[data-region="controls"] [data-slot="primary-action"]')).toBeNull()
  })

  it.each([
    ['the waiting line', false, /^Waiting for/],
    ['the prompt text', true, /^You are responding to/],
  ] as const)('announces %s politely', (_what, decides, words) => {
    renderTable({
      seats: 2,
      state: (state) => ({ ...state, pending: decides ? state.pending : null }),
    })
    const line = screen.getByText(words)
    expect(line).toHaveAttribute('aria-live', 'polite')
  })
})
