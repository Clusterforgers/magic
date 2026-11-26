// Goal: the shell frames every page with the brand, a main navigation (Play at /, Create at /new) that marks the current page and offers "Decks", "My games", "Tournaments", "Review cards" (with a badge of the distinct cards of your decks to read, echoed on the home page) and "Card support" only to an account, a window-high frame under the header for a page that fits it, a theme menu that changes the page's theme and remembers it, a way past the header by keyboard, a notice while the browser is offline, and the account preferences kept in use and saved on a page without the header (the game table).
import { act, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useFullBleed } from '@/components/appChrome'
import { appTheme } from '@/hooks/useTheme'
import { clearFlash, showFlash } from '@/lib/flash'
import { prefsApi } from '@/net/prefs'
import { AUTO_CHOICES_KEY, flowPrefs, saveAutoChoices } from '@/play/flowPrefs'
import { DEFAULT_HAND_SORT } from '@/play/handSort'
import { listed } from '@/test-utils/accountsServer'
import type { ReadState, UserPrefs } from '@/protocol/generated'
import { renderApp } from '@/test-utils/appHarness'
import { jsonResponse } from '@/test-utils/fakeFetch'

beforeEach(() => {
  appTheme().setChoice('system')
})

afterEach(() => {
  appTheme().setChoice('system')
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

const ADA = { username: 'Ada_L', password: 'correct horse battery' }
const mainNav = () => screen.getByRole('navigation', { name: 'Main' })

async function open(url = '/new', signedIn = false) {
  const app = await renderApp(url, {
    accounts: {
      users: { [ADA.username]: ADA.password },
      signedIn: signedIn ? ADA.username : undefined,
      games: [listed()],
    },
  })
  await waitFor(() => expect(app.store.getState().status).not.toBe('loading'))
  return app
}

describe('a notice another page left behind', () => {
  afterEach(() => act(() => clearFlash()))

  it('shows under the header on the page that is open, and goes with Dismiss', async () => {
    const app = await open('/')
    expect(screen.queryByRole('alert')).toBeNull()
    act(() => showFlash('You left the game.'))
    expect(screen.getByRole('alert')).toHaveTextContent('You left the game.')
    await app.user.click(screen.getByRole('button', { name: 'Dismiss' }))
    expect(screen.queryByRole('alert')).toBeNull()
  })
})

describe('the main navigation', () => {
  it('has the brand, Play, Precons and Create for a guest, and no Decks and no My games', async () => {
    await open()
    expect(screen.getByRole('link', { name: 'Magic' })).toHaveAttribute('href', '/')
    const links = within(mainNav()).getAllByRole('link')
    expect(links.map((link) => link.textContent)).toEqual(['Play', 'Precons', 'Create'])
    expect(links.map((link) => link.getAttribute('href'))).toEqual(['/', '/precons', '/new'])
  })

  it('keeps the tabs in a strip that takes the room left, never scrolls, with Create outside it', async () => {
    await open('/', true)
    const strip = mainNav().querySelector('[data-slot="nav-tabs"]')
    expect(strip).toHaveClass('min-w-0', 'flex-1')
    expect(strip).not.toHaveClass('overflow-x-auto')
    const create = within(mainNav()).getByRole('link', { name: 'Create' })
    expect(strip?.contains(create)).toBe(false)
    expect(create).toHaveClass('shrink-0')
    expect(
      within(mainNav())
        .getAllByRole('link')
        .filter((link) => strip?.contains(link)),
    ).toHaveLength(8)
    expect(within(mainNav()).queryByRole('button', { name: /^More pages/ })).toBeNull()
  })

  it('adds Decks, My games, Tournaments, Delves, Review cards and Card support, between Precons and Create, for an account', async () => {
    await open('/', true)
    const links = within(mainNav()).getAllByRole('link')
    expect(links.map((link) => link.textContent)).toEqual([
      'Play',
      'Precons',
      'Decks',
      'My games',
      'Tournaments',
      'Delves',
      'Review cards',
      'Card support',
      'Create',
    ])
    expect(links.map((link) => link.getAttribute('href'))).toEqual([
      '/',
      '/precons',
      '/decks',
      '/games',
      '/tournaments',
      '/delves',
      '/review?mine=true',
      '/support',
      '/new',
    ])
  })

  it('marks the page you are on, and only that one', async () => {
    const app = await open('/games', true)
    await screen.findByRole('heading', { name: 'My games' })
    expect(within(mainNav()).getByRole('link', { name: 'My games' })).toHaveAttribute('aria-current', 'page')
    expect(within(mainNav()).getByRole('link', { name: 'Play' })).not.toHaveAttribute('aria-current')
    await app.user.click(within(mainNav()).getByRole('link', { name: 'Play' }))
    await waitFor(() => expect(app.router.state.location.pathname).toBe('/'))
    expect(within(mainNav()).getByRole('link', { name: 'Play' })).toHaveAttribute('aria-current', 'page')
    expect(within(mainNav()).getByRole('link', { name: 'My games' })).not.toHaveAttribute('aria-current')
  })

  it('opens the page of a link without a reload', async () => {
    const app = await open('/games', true)
    await screen.findByRole('heading', { name: 'My games' })
    await app.user.click(within(mainNav()).getByRole('link', { name: 'Create' }))
    expect(await screen.findByRole('form', { name: 'Create a game' })).toBeInTheDocument()
  })
})

describe('the Review cards tab', () => {
  const waiting = (oracleId: string, inDecks: boolean, state: ReadState = 'pending') => ({
    oracle_id: oracleId,
    name: oracleId,
    read_hash: `h-${oracleId}`,
    state,
    differs: 0,
    rank: null,
    in_decks: inDecks,
  })

  async function openWith(cards: ReturnType<typeof waiting>[], signedIn = true) {
    const app = await renderApp('/', {
      accounts: { users: { [ADA.username]: ADA.password }, signedIn: signedIn ? ADA.username : undefined },
      rest: { reads: () => jsonResponse(200, { title: '', cards }) },
    })
    await waitFor(() => expect(app.store.getState().status).not.toBe('loading'))
    return app
  }
  const tab = () => within(mainNav()).getByRole('link', { name: /^Review cards/ })

  it.each([
    ['cards of your decks, each once', [waiting('a', true), waiting('b', true), waiting('a', true)], 2],
    ['only the cards your decks hold', [waiting('a', true), waiting('x', false), waiting('y', false)], 1],
    ['a pending draft only', [waiting('a', true), waiting('b', true, 'approved')], 1],
  ])('counts %s on a badge named in the label', async (_case, cards, count) => {
    await openWith(cards)
    await waitFor(() => expect(tab()).toHaveAccessibleName(`Review cards, ${count} to read`))
    expect(tab().querySelector('[data-slot="to-read-badge"]')).toHaveTextContent(String(count))
    expect(tab()).toHaveAttribute('href', '/review?mine=true')
  })

  it('is the current page on the review of your decks, and Decks is not', async () => {
    await renderApp('/review?mine=true', {
      accounts: { users: { [ADA.username]: ADA.password }, signedIn: ADA.username },
    })
    await waitFor(() => expect(tab()).toHaveAttribute('aria-current', 'page'))
    expect(within(mainNav()).getByRole('link', { name: 'Decks' })).not.toHaveAttribute('aria-current')
  })

  it('shows no badge when none of your cards waits', async () => {
    const app = await openWith([waiting('x', false)])
    await app.settle()
    expect(tab()).toHaveAccessibleName('Review cards')
    expect(tab().querySelector('[data-slot="to-read-badge"]')).toBeNull()
  })

  it('shows 99+ for a long list', async () => {
    await openWith(Array.from({ length: 120 }, (_, at) => waiting(`c${at}`, true)))
    await waitFor(() => expect(tab()).toHaveAccessibleName('Review cards, 120 to read'))
    expect(tab().querySelector('[data-slot="to-read-badge"]')).toHaveTextContent('99+')
  })

  it('is not there for a guest, who is not asked for the count', async () => {
    const app = await openWith([waiting('a', true)], false)
    await app.settle()
    expect(within(mainNav()).queryByRole('link', { name: /^Review cards/ })).toBeNull()
  })

  it('tells the home page to point there while cards wait', async () => {
    await openWith([waiting('a', true), waiting('b', true)])
    const notice = await screen.findByRole('link', { name: '2 cards wait for your read' })
    expect(notice).toHaveAttribute('href', '/review?mine=true')
  })

  it('leaves the home page without the notice when none waits', async () => {
    const app = await openWith([])
    await app.settle()
    expect(screen.queryByRole('link', { name: /wait for your read/ })).toBeNull()
  })
})

describe('the More menu of a row too narrow for the tabs', () => {
  /** Every measured tab and the More button 100 px wide, in a strip of `room` px. */
  function narrow(room: number) {
    vi.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function (this: Element) {
      const width = this.hasAttribute('data-measure') ? 100 : 0
      return { x: 0, y: 0, left: 0, top: 0, right: width, bottom: 0, width, height: 0, toJSON: () => ({}) }
    })
    vi.spyOn(Element.prototype, 'clientWidth', 'get').mockImplementation(function (this: Element) {
      return this.getAttribute('data-slot') === 'nav-tabs' ? room : 0
    })
  }
  const more = () => within(mainNav()).getByRole('button', { name: /^More pages/ })
  const rowNames = () =>
    within(mainNav())
      .getAllByRole('link')
      .map((link) => link.textContent)

  it.each([
    [
      450,
      ['Play', 'Precons', 'Decks', 'Create'],
      ['My games', 'Tournaments', 'Delves', 'Review cards', 'Card support'],
    ],
    [
      750,
      ['Play', 'Precons', 'Decks', 'My games', 'Tournaments', 'Delves', 'Create'],
      ['Review cards', 'Card support'],
    ],
    [
      150,
      ['Create'],
      ['Play', 'Precons', 'Decks', 'My games', 'Tournaments', 'Delves', 'Review cards', 'Card support'],
    ],
  ])('in %i px keeps the tabs that fit in order and lists the rest under More', async (room, row, menu) => {
    narrow(room)
    const app = await open('/', true)
    await waitFor(() => expect(rowNames()).toEqual(row))
    await app.user.click(more())
    const items = await screen.findAllByRole('menuitem')
    expect(items.map((item) => item.textContent)).toEqual(menu)
  })

  it('shows every tab and no More where they all fit', async () => {
    narrow(900)
    await open('/', true)
    await waitFor(() => expect(rowNames()).toHaveLength(9))
    expect(within(mainNav()).queryByRole('button', { name: /^More pages/ })).toBeNull()
  })

  it('opens a page from the menu without a reload, closes, and marks More while that page is open', async () => {
    narrow(450)
    const app = await open('/', true)
    await waitFor(() => expect(more()).toHaveAccessibleName('More pages'))
    expect(more()).not.toHaveAttribute('data-current')
    await app.user.click(more())
    await app.user.click(await screen.findByRole('menuitem', { name: 'Delves' }))
    await waitFor(() => expect(app.router.state.location.pathname).toBe('/delves'))
    await waitFor(() => expect(screen.queryByRole('menu')).toBeNull())
    expect(more()).toHaveAttribute('data-current')
    expect(more()).toHaveAccessibleName('More pages, the current page is one of them')
    await app.user.click(more())
    expect(await screen.findByRole('menuitem', { name: 'Delves' })).toHaveAttribute('aria-current', 'page')
  })

  it('is operable from the keyboard: Enter opens it on the first page, Escape closes it back on the button', async () => {
    narrow(450)
    const app = await open('/', true)
    await waitFor(() => expect(more()).toBeInTheDocument())
    more().focus()
    await app.user.keyboard('{Enter}')
    const first = await screen.findByRole('menuitem', { name: 'My games' })
    await waitFor(() => expect(first).toHaveFocus())
    await app.user.keyboard('{ArrowDown}')
    expect(screen.getByRole('menuitem', { name: 'Tournaments' })).toHaveFocus()
    await app.user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('menu')).toBeNull())
    expect(more()).toHaveFocus()
  })

  it('counts the cards to read on More while Review cards is in it', async () => {
    narrow(450)
    await renderApp('/', {
      accounts: { users: { [ADA.username]: ADA.password }, signedIn: ADA.username },
      rest: {
        reads: () =>
          jsonResponse(200, {
            title: '',
            cards: [
              {
                oracle_id: 'a',
                name: 'a',
                read_hash: 'h',
                state: 'pending',
                differs: 0,
                rank: null,
                in_decks: true,
              },
            ],
          }),
      },
    })
    await waitFor(() => expect(more()).toHaveAccessibleName('More pages, 1 card to read'))
    expect(more()).toHaveTextContent('1')
  })
})

describe('the way past the header', () => {
  it('is a first stop that moves the focus into the page, without touching the address', async () => {
    const app = await open('/new')
    await screen.findByRole('form', { name: 'Create a game' })
    await app.user.tab()
    const skip = screen.getByRole('button', { name: 'Skip to content' })
    expect(skip).toHaveFocus()
    const before = app.router.state.location.href
    await app.user.keyboard('{Enter}')
    expect(screen.getByRole('main')).toHaveFocus()
    expect(app.router.state.location.href).toBe(before)
  })
})

describe('the theme menu', () => {
  it('shows the choice in force and offers Light, Dark and System', async () => {
    const app = await open()
    await app.user.click(screen.getByRole('button', { name: 'Theme: System' }))
    const items = await screen.findAllByRole('menuitemradio')
    expect(items.map((item) => item.textContent)).toEqual(['Light', 'Dark', 'System'])
    expect(items.map((item) => item.getAttribute('aria-checked'))).toEqual(['false', 'false', 'true'])
  })

  it('puts the page in the dark theme, remembers it and shows it on the button', async () => {
    const app = await open()
    expect(document.documentElement).not.toHaveClass('dark')
    await app.user.click(screen.getByRole('button', { name: 'Theme: System' }))
    await app.user.click(await screen.findByRole('menuitemradio', { name: 'Dark' }))
    expect(document.documentElement).toHaveClass('dark')
    expect(window.localStorage.getItem('theme')).toBe('dark')
    expect(screen.getByRole('button', { name: 'Theme: Dark' })).toBeInTheDocument()
  })

  it('goes back to the system, which forgets the choice', async () => {
    const app = await open()
    appTheme().setChoice('dark')
    await app.user.click(await screen.findByRole('button', { name: 'Theme: Dark' }))
    await app.user.click(await screen.findByRole('menuitemradio', { name: 'System' }))
    expect(document.documentElement).not.toHaveClass('dark')
    expect(window.localStorage.getItem('theme')).toBeNull()
  })

  it('is operable from the keyboard', async () => {
    const app = await open()
    screen.getByRole('button', { name: 'Theme: System' }).focus()
    await app.user.keyboard('{Enter}')
    const light = await screen.findByRole('menuitemradio', { name: 'Light' })
    await act(async () => light.focus())
    await app.user.keyboard('{Enter}')
    expect(appTheme().getChoice()).toBe('light')
    expect(await screen.findByRole('button', { name: 'Theme: Light' })).toHaveFocus()
  })
})

describe('while the browser is offline', () => {
  it('says so politely, and stops saying so when the network is back', async () => {
    const onLine = vi.spyOn(window.navigator, 'onLine', 'get').mockReturnValue(false)
    await open()
    expect(screen.getByRole('status')).toHaveTextContent('You are offline')
    onLine.mockReturnValue(true)
    act(() => {
      window.dispatchEvent(new Event('online'))
    })
    await waitFor(() => expect(screen.queryByText('You are offline')).toBeNull())
  })

  it('is silent while the browser is online', async () => {
    await open()
    expect(screen.queryByText('You are offline')).toBeNull()
  })
})

describe('the pages of Play and Create', () => {
  it('marks Create on the create page, and Play on Play', async () => {
    const app = await open('/new')
    await screen.findByRole('form', { name: 'Create a game' })
    expect(within(mainNav()).getByRole('link', { name: 'Create' })).toHaveAttribute('aria-current', 'page')
    expect(within(mainNav()).getByRole('link', { name: 'Play' })).not.toHaveAttribute('aria-current')
    await app.user.click(within(mainNav()).getByRole('link', { name: 'Play' }))
    await screen.findByRole('heading', { level: 1, name: 'Play' })
    expect(within(mainNav()).getByRole('link', { name: 'Play' })).toHaveAttribute('aria-current', 'page')
    expect(within(mainNav()).getByRole('link', { name: 'Create' })).not.toHaveAttribute('aria-current')
  })

  it('is the window high under its header on Play, which fits it, and a growing page elsewhere', async () => {
    const app = await open('/')
    await screen.findByRole('heading', { level: 1, name: 'Play' })
    const shell = screen.getByRole('main').parentElement as HTMLElement
    expect(shell).toHaveAttribute('data-fit')
    expect(shell).not.toHaveAttribute('data-bleed')
    expect(shell).toHaveClass('h-dvh')
    expect(mainNav()).toBeInTheDocument()
    await app.user.click(within(mainNav()).getByRole('link', { name: 'Create' }))
    await screen.findByRole('form', { name: 'Create a game' })
    expect(shell).not.toHaveAttribute('data-fit')
    expect(shell).toHaveClass('min-h-dvh')
    expect(shell).not.toHaveClass('h-dvh')
  })
})

describe('the account preferences', () => {
  function Table() {
    useFullBleed()
    return <p>the game table</p>
  }

  const kept: UserPrefs = {
    default_stops: [],
    end_turn_by_hand: true,
    auto_choices: { order_triggers: true, assign_damage: false, smart_stops: false },
    auto_yields: [],
    hand_sort: DEFAULT_HAND_SORT,
  }

  it('stay the account’s on a page without the header (the game table), so a change made there is saved to it', async () => {
    const load = vi.spyOn(prefsApi, 'load').mockResolvedValue(kept)
    const save = vi.spyOn(prefsApi, 'save').mockImplementation((prefs) => Promise.resolve(prefs))
    await open('/', true)
    await waitFor(() => expect(flowPrefs().endTurnByHand).toBe(true))
    const table = render(<Table />)
    await waitFor(() => expect(screen.queryByRole('navigation', { name: 'Main' })).toBeNull())
    expect(flowPrefs().autoChoices.order_triggers).toBe(true)
    act(() => saveAutoChoices({ ...flowPrefs().autoChoices, auto_pay: true }))
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1))
    expect(save.mock.calls[0]?.[0].auto_choices.auto_pay).toBe(true)
    expect(window.localStorage.getItem(AUTO_CHOICES_KEY)).toBeNull()
    act(() => table.unmount())
    await screen.findByRole('navigation', { name: 'Main' })
    expect(flowPrefs().autoChoices.auto_pay).toBe(true)
    expect(load).toHaveBeenCalledTimes(1)
  })
})
