// Goal: the table draws every seat at the right place for tables of 2 to 12 with the viewer at the bottom, opens in grid mode up to 4 seats and focus mode from 5, switches between them from the toolbar, focuses an opponent from a tile by mouse or keyboard, follows the turn until told otherwise, and is usable from the keyboard.
import { act, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { indexById } from '@/play/view'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from './dev/fixture'
import { opponentsOf } from './layout'

const COUNTS = Array.from({ length: 11 }, (_, index) => index + 2)

const panels = (container: HTMLElement): number[] =>
  [...container.querySelectorAll('[data-seat]')].map((element) => Number(element.getAttribute('data-seat')))
const tiles = (container: HTMLElement): number[] =>
  [...container.querySelectorAll('[data-tile-seat]')].map((element) =>
    Number(element.getAttribute('data-tile-seat')),
  )
const tile = (container: HTMLElement, seat: number): HTMLElement =>
  must(container.querySelector<HTMLElement>(`[data-tile-seat="${seat}"]`))
const layout = (container: HTMLElement): string | null =>
  container.querySelector('[data-layout]')?.getAttribute('data-layout') ?? null

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('grid mode', () => {
  it.each(COUNTS)('draws every opponent in turn order and then the viewer last at a table of %i', (count) => {
    for (const viewer of [0, count - 1]) {
      const fixture = buildFixture({ seats: count, viewer, permanents: 2 })
      const { container, unmount } = renderTable({ fixture, options: { mode: 'grid' } })
      expect(panels(container), `viewer ${viewer}`).toEqual([...opponentsOf(count, viewer), viewer])
      expect(tiles(container)).toEqual([])
      expect(layout(container)).toBe('grid')
      unmount()
    }
  })

  it.each([
    [2, '1'],
    [3, '2'],
    [4, '3'],
  ])('opens a table of %i seats in grid mode with %s column(s)', (seats, columns) => {
    const { container } = renderTable({ seats })
    expect(layout(container)).toBe('grid')
    expect(container.querySelector('[data-layout]')).toHaveAttribute('data-columns', columns)
  })

  it('puts the viewer at the bottom, below the battlefields of the opponents and the stack', () => {
    const { container } = renderTable({ seats: 3 })
    const order = [...container.querySelectorAll('[data-seat], [aria-label="Stack"]')].map(
      (element) => element.getAttribute('data-seat') ?? 'stack',
    )
    expect(order).toEqual(['1', '2', 'stack', '0'])
  })

  it('turns the viewer around: seat 2 of 4 sees 3, 0, 1 above itself', () => {
    const { container } = renderTable({ fixture: buildFixture({ seats: 4, viewer: 2, permanents: 2 }) })
    expect(panels(container)).toEqual([3, 0, 1, 2])
  })

  it("draws every opponent's battlefield in full, in rows of lands, other permanents and creatures", () => {
    renderTable({ seats: 2 })
    const rows = screen.getAllByRole('list').map((list) => list.getAttribute('aria-label'))
    const bo = rows.filter((label) => label?.startsWith("Bo's"))
    expect(bo).toEqual(["Bo's command zone", "Bo's lands", "Bo's other permanents", "Bo's creatures"])
    const alice = rows.filter((label) => label?.startsWith("Alice's"))
    expect(alice).toEqual([
      "Alice's creatures",
      "Alice's other permanents",
      "Alice's lands",
      "Alice's command zone",
      "Alice's hand",
    ])
  })
})

describe('focus mode', () => {
  it.each(COUNTS)('draws one large opponent, a tile for each and the viewer at a table of %i', (count) => {
    for (const viewer of [0, count - 1]) {
      const fixture = buildFixture({ seats: count, viewer, permanents: 2 })
      const { container, unmount } = renderTable({ fixture, options: { mode: 'focus' } })
      const opponents = opponentsOf(count, viewer)
      expect(tiles(container), `viewer ${viewer}`).toEqual(opponents)
      expect(panels(container)).toHaveLength(2)
      expect(panels(container).at(-1)).toBe(viewer)
      expect(opponents).toContain(panels(container)[0])
      expect(layout(container)).toBe('focus')
      unmount()
    }
  })

  it.each([5, 8, 12])('opens a table of %i seats in focus mode', (seats) => {
    const { container } = renderTable({ fixture: buildFixture({ seats, permanents: 2 }) })
    expect(layout(container)).toBe('focus')
  })

  it('shows the player whose turn it is, large, to begin with', () => {
    const { container } = renderTable({ fixture: buildFixture({ seats: 6, permanents: 2, active: 4 }) })
    expect(panels(container)).toEqual([4, 0])
    expect(tile(container, 4)).toHaveAttribute('aria-current', 'true')
    expect(tile(container, 1)).not.toHaveAttribute('aria-current')
  })

  it('shows every other seat as a tile with a name for a screen reader', () => {
    const { container } = renderTable({ fixture: buildFixture({ seats: 6, permanents: 2 }) })
    expect(within(screen.getByRole('list', { name: 'Opponents' })).getAllByRole('listitem')).toHaveLength(5)
    expect(tile(container, 3)).toHaveAccessibleName("Show Dee's board. Life 34, 5 cards in hand")
    expect(tile(container, 1)).toHaveAccessibleName(
      "Show Bo's board. Life 38, 6 cards in hand, active turn, shown now",
    )
  })

  it('marks an eliminated seat on its tile', () => {
    const { container } = renderTable({
      fixture: buildFixture({ seats: 6, permanents: 2 }),
      state: (state) => {
        const view = clone(must(state.view))
        must(view.players[3]).lost = 'ZeroLife'
        return { ...state, view }
      },
    })
    expect(tile(container, 3)).toHaveTextContent('Eliminated')
    expect(tile(container, 3)).toHaveAccessibleName(expect.stringContaining('eliminated'))
  })

  it('opens on the options it is given and ignores a focus that is not an opponent', () => {
    const fixture = buildFixture({ seats: 6, permanents: 2 })
    expect(panels(renderTable({ fixture, options: { mode: 'focus', focus: 3 } }).container)).toEqual([3, 0])
    expect(panels(renderTable({ fixture, options: { mode: 'focus', focus: 0 } }).container)).toContain(1)
  })
})

describe('choosing the opponent to focus', () => {
  it('focuses a tile that is clicked', async () => {
    const { container, user } = renderTable({ fixture: buildFixture({ seats: 6, permanents: 2 }) })
    await user.click(tile(container, 3))
    expect(panels(container)).toEqual([3, 0])
    expect(tile(container, 3)).toHaveAttribute('aria-current', 'true')
    expect(tile(container, 1)).not.toHaveAttribute('aria-current')
  })

  it.each([
    ['Enter', '{Enter}'],
    ['Space', ' '],
  ])('focuses a tile when %s is pressed on it', async (_name, key) => {
    const { container, user } = renderTable({ fixture: buildFixture({ seats: 6, permanents: 2 }) })
    act(() => tile(container, 4).focus())
    await user.keyboard(key)
    expect(panels(container)).toEqual([4, 0])
    expect(tile(container, 4)).toHaveAttribute('aria-current', 'true')
  })

  it('reaches the tiles with Tab, after the layout buttons', async () => {
    const { container, user } = renderTable({ fixture: buildFixture({ seats: 5, permanents: 2 }) })
    await user.tab()
    expect(screen.getByRole('button', { name: 'Grid' })).toHaveFocus()
    await user.tab()
    expect(screen.getByRole('button', { name: 'Focus' })).toHaveFocus()
    await user.tab()
    expect(tile(container, 1)).toHaveFocus()
    await user.tab()
    expect(tile(container, 2)).toHaveFocus()
    await user.keyboard('{Enter}')
    expect(panels(container)).toEqual([2, 0])
  })

  it('follows the turn until a tile is chosen, then stays until told to follow again', async () => {
    const { container, user, update } = renderTable({ fixture: buildFixture({ seats: 6, permanents: 2 }) })
    const turnOf = (seat: number): void =>
      update((state) => {
        const view = clone(must(state.view))
        view.turn.active = seat
        return { view }
      })
    expect(screen.queryByRole('button', { name: 'Follow the turn' })).toBeNull()
    turnOf(4)
    expect(panels(container)).toEqual([4, 0])
    await user.click(tile(container, 2))
    turnOf(5)
    expect(panels(container)).toEqual([2, 0])
    await user.click(screen.getByRole('button', { name: 'Follow the turn' }))
    expect(panels(container)).toEqual([5, 0])
    expect(screen.queryByRole('button', { name: 'Follow the turn' })).toBeNull()
  })

  it('keeps showing a chosen opponent when the turn passes to the viewer', async () => {
    const { container, user, update } = renderTable({ fixture: buildFixture({ seats: 6, permanents: 2 }) })
    await user.click(tile(container, 3))
    update((state) => {
      const view = clone(must(state.view))
      view.turn.active = 0
      return { view }
    })
    expect(panels(container)).toEqual([3, 0])
  })

  it('falls back to the priority holder when the viewer is the active player and nobody was chosen', () => {
    const { container } = renderTable({
      fixture: buildFixture({ seats: 6, permanents: 2, active: 0 }),
      state: (state) => {
        const view = clone(must(state.view))
        view.priority = { holder: 5, passed: [] }
        return { ...state, view }
      },
    })
    expect(panels(container)).toEqual([5, 0])
  })
})

describe('the layout toggle', () => {
  it('switches a small table to focus mode and back', async () => {
    const { container, user } = renderTable({ fixture: buildFixture({ seats: 3, permanents: 2 }) })
    const grid = screen.getByRole('button', { name: 'Grid' })
    const focus = screen.getByRole('button', { name: 'Focus' })
    expect(grid).toHaveAttribute('aria-pressed', 'true')
    expect(focus).toHaveAttribute('aria-pressed', 'false')
    await user.click(focus)
    expect(layout(container)).toBe('focus')
    expect(focus).toHaveAttribute('aria-pressed', 'true')
    expect(grid).toHaveAttribute('aria-pressed', 'false')
    expect(tiles(container)).toEqual([1, 2])
    expect(panels(container)).toEqual([1, 0])
    await user.click(grid)
    expect(layout(container)).toBe('grid')
    expect(panels(container)).toEqual([1, 2, 0])
  })

  it('switches a large table to grid mode, which draws every opponent', async () => {
    const { container, user } = renderTable({ fixture: buildFixture({ seats: 7, permanents: 2 }) })
    expect(layout(container)).toBe('focus')
    await user.click(screen.getByRole('button', { name: 'Grid' }))
    expect(panels(container)).toEqual([1, 2, 3, 4, 5, 6, 0])
    expect(container.querySelector('[data-layout]')).toHaveAttribute('data-columns', '3')
  })

  it('is a labelled toolbar', () => {
    renderTable({ seats: 2 })
    const toolbar = screen.getByRole('toolbar', { name: 'Table layout' })
    expect(within(toolbar).getAllByRole('button')).toHaveLength(2)
  })
})

describe('compact cards', () => {
  const sizeOfCreatures = (container: HTMLElement, seat: number): (string | null)[] =>
    [
      ...container.querySelectorAll(
        `[data-battlefield-seat="${seat}"] ul[aria-label$="creatures"] [data-size]`,
      ),
    ].map((element) => element.getAttribute('data-size'))

  it.each([
    [7, 'normal'],
    [8, 'compact'],
    [12, 'compact'],
  ])('draws creatures at a table of %i seats in the %s size', (seats, size) => {
    const { container } = renderTable({ fixture: buildFixture({ seats, permanents: 4 }) })
    const sizes = sizeOfCreatures(container, 0)
    expect(sizes.length).toBeGreaterThan(0)
    expect(new Set(sizes)).toEqual(new Set([size]))
  })

  it('always draws lands compact, and normal creatures at a small table', () => {
    const { container } = renderTable({ seats: 2 })
    const lands = [
      ...container.querySelectorAll('[data-battlefield-seat="0"] ul[aria-label$="lands"] [data-size]'),
    ]
    expect(lands.length).toBe(16)
    expect(new Set(lands.map((element) => element.getAttribute('data-size')))).toEqual(new Set(['compact']))
    expect(new Set(sizeOfCreatures(container, 0))).toEqual(new Set(['normal']))
  })
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
      'Stack',
      'Bo',
      'Cy',
      'Alice (you)',
      "Bo's battlefield",
      "Alice's battlefield",
    ]) {
      expect(screen.getByRole('region', { name }), name).toBeInTheDocument()
    }
    expect(screen.getByRole('complementary', { name: 'Game log and chat' })).toBeInTheDocument()
    expect(screen.getByRole('toolbar', { name: 'Table layout' })).toBeInTheDocument()
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

describe('turn banner and decision line', () => {
  it('prints the turn, the priority holder and the decision this seat faces', () => {
    renderTable({ seats: 4 })
    const strip = screen.getByRole('region', { name: 'Turn and priority' })
    expect(within(strip).getByRole('heading', { name: 'Turn 5 · Bo · Declare blockers' })).toBeInTheDocument()
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

  it('is read-only: there is nothing in the strip to press', () => {
    renderTable({ seats: 4 })
    expect(
      within(screen.getByRole('region', { name: 'Turn and priority' })).queryAllByRole('button'),
    ).toEqual([])
  })

  it('announces a change of decision politely', () => {
    renderTable({ seats: 2 })
    const line = screen.getByText('Your decision: Priority')
    expect(line).toHaveAttribute('aria-live', 'polite')
  })
})

describe('the drawer', () => {
  const drawer = () => screen.getByRole('complementary', { name: 'Game log and chat' })

  it('opens on the log with the chat a tab away', async () => {
    const { user } = renderTable({ seats: 2 })
    expect(within(drawer()).getByRole('log', { name: 'Game log' })).toBeInTheDocument()
    expect(within(drawer()).queryByRole('log', { name: 'Chat' })).toBeNull()
    await user.click(within(drawer()).getByRole('tab', { name: 'Chat' }))
    expect(within(drawer()).getByRole('log', { name: 'Chat' })).toBeInTheDocument()
    expect(within(drawer()).queryByRole('log', { name: 'Game log' })).toBeNull()
  })

  it('collapses and expands from its button', async () => {
    const { user } = renderTable({ seats: 2 })
    const toggle = within(drawer()).getByRole('button', { name: 'Collapse log and chat' })
    expect(toggle).toHaveAttribute('aria-expanded', 'true')
    await user.click(toggle)
    expect(within(drawer()).queryByRole('log')).toBeNull()
    const open = within(drawer()).getByRole('button', { name: 'Expand log and chat' })
    expect(open).toHaveAttribute('aria-expanded', 'false')
    await user.click(open)
    expect(within(drawer()).getByRole('log', { name: 'Game log' })).toBeInTheDocument()
  })

  it('starts folded on a narrow screen, and a tab opens it', async () => {
    vi.stubGlobal('matchMedia', () => ({ matches: false }))
    const { user } = renderTable({ seats: 2 })
    expect(within(drawer()).queryByRole('log')).toBeNull()
    await user.click(within(drawer()).getByRole('tab', { name: 'Chat' }))
    expect(within(drawer()).getByRole('log', { name: 'Chat' })).toBeInTheDocument()
  })

  it('stays open from the first render on a wide screen', () => {
    vi.stubGlobal('matchMedia', () => ({ matches: true }))
    renderTable({ seats: 2 })
    expect(within(drawer()).getByRole('log', { name: 'Game log' })).toBeInTheDocument()
  })
})
