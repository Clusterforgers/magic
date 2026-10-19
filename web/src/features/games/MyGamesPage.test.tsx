// Goal: the My games page sends a visitor who is not signed in to the sign-in form and back, and every state of the signed-in page is designed in the Play page's wide column: a skeleton only after 300 ms, an empty state with two ways on, an error with Retry that keeps old rows dimmed, and a table (stacked rows on a narrow screen) of state badges, results and a Continue link only where a game can be continued.
import { screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { AuthProvider } from '@/features/auth/AuthProvider'
import { createAuthStore } from '@/features/auth/store'
import type { MyGame } from '@/protocol/generated'
import { authError, listed, startAccounts } from '@/test-utils/accountsServer'
import type { AccountsScript } from '@/test-utils/accountsServer'
import { renderApp } from '@/test-utils/appHarness'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { renderInRouter } from '@/test-utils/router'
import { MyGamesPage } from './MyGamesPage'

afterEach(() => {
  vi.unstubAllGlobals()
})

const ADA = { username: 'Ada_L', password: 'correct horse battery' }
const users = { [ADA.username]: ADA.password }
const NOW = Date.parse('2026-10-18T12:00:00Z')
const ago = (minutes: number): string =>
  new Date(NOW - minutes * 60_000).toISOString().replace(/\.\d{3}Z$/, 'Z')

const TABLE = { name: 'Your games, newest first' }
const table = () => screen.getByRole('table', TABLE)
/** The game rows of the table, without its header row. */
const rows = () =>
  within(table())
    .getAllByRole('row')
    .filter((row) => row.hasAttribute('data-game'))
const gameOf = (row: HTMLElement | undefined): string | null => row?.getAttribute('data-game') ?? null
const ids = () => rows().map(gameOf)
const page = () => screen.getByRole('heading', { level: 1, name: 'My games' }).closest('[data-slot="page"]')
const refresh = () => screen.getByRole('button', { name: 'Refresh' })

/** A window under the table breakpoint, where each game is a stacked row. */
function narrowWindow() {
  vi.stubGlobal('matchMedia', (query: string) => ({
    matches: false,
    media: query,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
  }))
}

/** The real app at `/games`, signed in as Ada unless told otherwise. */
async function openApp(accounts: AccountsScript = {}, strict = false) {
  const app = await renderApp('/games', { accounts: { users, signedIn: ADA.username, ...accounts }, strict })
  return app
}

/** Just the page, in a minimal router, on a fixed clock. */
async function openRows(games: MyGame[]) {
  const server = startAccounts({ users, signedIn: ADA.username, games })
  const store = createAuthStore({ api: server.api })
  await renderInRouter(
    <AuthProvider store={store}>
      <MyGamesPage clock={() => NOW} />
    </AuthProvider>,
  )
  await screen.findByRole('table', TABLE)
  return { server, store }
}

describe('a visitor who is not signed in', () => {
  it('is sent to the sign-in form, which remembers where they were going', async () => {
    const app = await renderApp('/games', { accounts: { users } })
    expect(await screen.findByRole('form', { name: 'Sign in' })).toBeInTheDocument()
    expect(app.router.state.location.pathname).toBe('/login')
    expect(app.router.state.matches.at(-1)?.search).toEqual({ next: '/games' })
    expect(app.accounts.calls.map((call) => call.url)).not.toContain('/api/me/games')
  })

  it('comes back to the list after signing in', async () => {
    const app = await renderApp('/games', {
      accounts: { users, games: [listed({ others: [{ seat: 1, name: 'Bo', user: null }] })] },
    })
    await screen.findByRole('form', { name: 'Sign in' })
    await app.user.type(screen.getByLabelText('Username'), ADA.username)
    await app.user.type(screen.getByLabelText('Password'), ADA.password)
    await app.user.click(screen.getByRole('button', { name: 'Sign in' }))
    expect(await screen.findByRole('table', TABLE)).toBeInTheDocument()
    expect(app.router.state.location.pathname).toBe('/games')
    expect(rows()).toHaveLength(1)
  })

  it('does not go back to the list through the history, so Back does not bounce off the redirect', async () => {
    const app = await renderApp('/games', { accounts: { users } })
    await screen.findByRole('form', { name: 'Sign in' })
    app.router.history.back()
    await app.settle()
    expect(app.router.state.location.pathname).not.toBe('/games')
  })
})

describe('before the server has said who is signed in', () => {
  it('says it is checking, and shows no list and no redirect', async () => {
    const app = await renderApp('/games', {
      accounts: { users, override: { me: () => new Promise<Response>(() => undefined) } },
    })
    expect(screen.getByRole('status')).toHaveTextContent('Checking your sign-in…')
    expect(app.router.state.location.pathname).toBe('/games')
    expect(screen.queryByRole('table')).toBeNull()
    expect(screen.queryByRole('list')).toBeNull()
  })
})

describe('a server without accounts', () => {
  it('explains that there is no list, and the header still points at creating a game', async () => {
    const app = await openApp({ unavailable: true })
    expect(
      await screen.findByText(/this server has no accounts, so it keeps no list of games/i),
    ).toBeInTheDocument()
    expect(screen.getByRole('heading', { level: 2, name: 'No list of games here' })).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Create game' })).toHaveAttribute('href', '/new')
    expect(app.accounts.calls.map((call) => call.url)).toEqual(['/api/auth/me'])
    expect(screen.queryByRole('navigation', { name: 'Account' })).toBeNull()
  })
})

describe('the page', () => {
  it('is titled, and asks the server for the games once the account is known', async () => {
    const app = await openApp({ games: [listed()] })
    expect(await screen.findByRole('heading', { level: 1, name: 'My games' })).toBeInTheDocument()
    expect(document.title).toBe('My games · Magic')
    await screen.findByRole('table', TABLE)
    expect(app.accounts.calls.filter((call) => call.url === '/api/me/games')).toHaveLength(1)
    expect(app.accounts.calls.find((call) => call.url === '/api/me/games')?.init?.credentials).toBe('include')
  })

  it('sits in the wide column of the Play page, under the header, with the way to a new game beside the title', async () => {
    await openApp({ games: [listed()] })
    await screen.findByRole('table', TABLE)
    expect(page()).toHaveAttribute('data-width', 'wide')
    expect(document.querySelector('[data-fit="true"]')).not.toBeNull()
    expect(within(page() as HTMLElement).getByRole('link', { name: 'Create game' })).toHaveAttribute(
      'href',
      '/new',
    )
  })

  it('keeps the same width in its loading state, before it knows who is signed in', async () => {
    await renderApp('/games', {
      accounts: { users, override: { me: () => new Promise<Response>(() => undefined) } },
    })
    expect(document.querySelector('[data-slot="page"]')).toHaveAttribute('data-width', 'wide')
  })

  it('says how many games it lists, politely', async () => {
    await openApp({ games: [listed({ game: 'a' }), listed({ game: 'b' })] })
    await screen.findByRole('table', TABLE)
    expect(screen.getByRole('status')).toHaveTextContent('2 games')
  })

  it('says it is loading while the first answer is awaited', async () => {
    const app = await openApp({ override: { games: () => new Promise<Response>(() => undefined) } })
    await waitFor(() => expect(app.store.getState().games.status).toBe('loading'))
    expect(document.querySelector('[data-slot="loading"]')).toHaveTextContent('Loading your games…')
    expect(refresh()).toHaveAttribute('aria-disabled', 'true')
    expect(screen.queryByRole('table')).toBeNull()
  })

  it('draws skeleton rows only once the wait has lasted 300 ms', async () => {
    const app = await openApp({ override: { games: () => new Promise<Response>(() => undefined) } })
    await waitFor(() => expect(app.store.getState().games.status).toBe('loading'))
    expect(document.querySelector('[data-slot="skeleton"]')).toBeNull()
    await waitFor(() => expect(document.querySelectorAll('[data-slot="skeleton"]')).toHaveLength(4), {
      timeout: 1500,
    })
    expect(document.querySelector('[data-slot="loading"]')).toHaveTextContent('Loading your games…')
  })

  it('lists the newest game first, whatever order the server used', async () => {
    const old = listed({
      game: 'old',
      created_at: ago(300),
      others: [{ seat: 1, name: 'Oldie', user: null }],
    })
    const fresh = listed({
      game: 'fresh',
      created_at: ago(2),
      others: [{ seat: 1, name: 'Fresh', user: null }],
    })
    const mid = listed({
      game: 'mid',
      created_at: ago(60),
      others: [{ seat: 1, name: 'Middle', user: null }],
    })
    await openApp({ games: [old, fresh, mid] })
    await screen.findByRole('table', TABLE)
    expect(ids()).toEqual(['fresh', 'mid', 'old'])
  })

  it('asks for the games at most twice when React mounts everything twice', async () => {
    const app = await openApp({ games: [listed()] }, true)
    await screen.findByRole('table', TABLE)
    expect(app.accounts.calls.filter((call) => call.url === '/api/me/games').length).toBeLessThanOrEqual(2)
    expect(rows()).toHaveLength(1)
  })
})

describe('with no games', () => {
  const empty = () =>
    screen.getByRole('heading', { name: 'You have no games yet' }).closest('[data-slot="empty-state"]')

  it('shows the empty state with two ways on: create a game or find an open one', async () => {
    await openApp({ games: [] })
    await screen.findByRole('heading', { name: 'You have no games yet' })
    expect(screen.queryByRole('table')).toBeNull()
    expect(screen.queryByRole('region', { name: 'Your games' })).toBeNull()
    const state = within(empty() as HTMLElement)
    expect(state.getByRole('link', { name: 'Create game' })).toHaveAttribute('href', '/new')
    expect(state.getByRole('link', { name: 'Find an open game' })).toHaveAttribute('href', '/')
    expect(screen.getByRole('status')).toHaveTextContent('No games')
  })

  it('opens the create page from the empty state', async () => {
    const app = await openApp({ games: [] })
    await screen.findByRole('heading', { name: 'You have no games yet' })
    await app.user.click(within(empty() as HTMLElement).getByRole('link', { name: 'Create game' }))
    expect(await screen.findByRole('form', { name: 'Create a game' })).toBeInTheDocument()
  })

  it('opens the Play page from the empty state', async () => {
    const app = await openApp({ games: [] })
    await screen.findByRole('heading', { name: 'You have no games yet' })
    await app.user.click(within(empty() as HTMLElement).getByRole('link', { name: 'Find an open game' }))
    await waitFor(() => expect(app.router.state.location.pathname).toBe('/'))
  })
})

describe('when loading fails', () => {
  it('says why in words with a title, offers a retry under the words, and the retry works', async () => {
    let broken = true
    const app = await openApp({
      games: [listed()],
      override: {
        games: () => (broken ? authError(503, 'unavailable') : jsonResponse(200, { games: [listed()] })),
      },
    })
    const alert = await screen.findByRole('alert')
    expect(within(alert).getByText('Could not load your games')).toBeInTheDocument()
    expect(alert).toHaveTextContent(/cannot check accounts right now/i)
    expect(alert.querySelector('svg')).toHaveAttribute('aria-hidden', 'true')
    // Under the words, in the column of the words, so the icon's column stays as wide as the icon.
    expect(within(alert).getByRole('button', { name: 'Retry' }).parentElement).toHaveClass('col-start-2')
    expect(screen.queryByRole('table')).toBeNull()
    broken = false
    await app.user.click(within(alert).getByRole('button', { name: 'Retry' }))
    expect(await screen.findByRole('table', TABLE)).toBeInTheDocument()
    expect(screen.queryByRole('alert')).toBeNull()
  })

  it.each([
    [
      'the network fails',
      () => Promise.reject(new TypeError('Failed to fetch')),
      /could not reach the server/i,
    ],
    ['the server fails', () => authError(500, 'internal'), /ran into a problem/i],
    ['a proxy answers with HTML', () => new Response('<html>', { status: 502 }), /status 502/],
    [
      'the list is malformed',
      () => jsonResponse(200, { games: 'none' }),
      /could not be read|could not read/i,
    ],
  ])('names the cause when %s', async (_what, answer, words) => {
    await openApp({ override: { games: answer } })
    expect(await screen.findByRole('alert')).toHaveTextContent(words)
  })

  it('keeps the rows it had, dimmed, and says the refresh failed', async () => {
    let broken = false
    const app = await openApp({
      games: [listed({ game: 'kept' })],
      override: {
        games: () =>
          broken ? authError(500, 'internal') : jsonResponse(200, { games: [listed({ game: 'kept' })] }),
      },
    })
    await screen.findByRole('table', TABLE)
    broken = true
    await app.user.click(refresh())
    const alert = await screen.findByRole('alert')
    expect(within(alert).getByText('Could not refresh your games')).toBeInTheDocument()
    expect(alert).toHaveTextContent('The games below may be out of date.')
    expect(ids()).toEqual(['kept'])
    expect(screen.getByRole('region', { name: 'Your games' })).toHaveAttribute('data-stale', 'true')
    expect(within(rows()[0] as HTMLElement).getByRole('rowheader')).toHaveClass('text-muted-foreground')
  })

  it('sends the visitor to the sign-in form when the session turns out to have ended', async () => {
    const app = await openApp({ games: [listed()] })
    await screen.findByRole('table', TABLE)
    await app.accounts.api.logout() // the cookie is gone: expired, or signed out in another tab
    await app.user.click(refresh())
    expect(await screen.findByRole('form', { name: 'Sign in' })).toBeInTheDocument()
    expect(app.router.state.matches.at(-1)?.search).toEqual({ next: '/games' })
    expect(app.store.getState()).toMatchObject({ status: 'signed-out', user: null })
  })
})

describe('refreshing', () => {
  it('asks again, says so meanwhile and shows what changed', async () => {
    let release!: (response: Response) => void
    let hold = false
    const app = await openApp({
      games: [listed({ game: 'first' })],
      override: {
        games: () =>
          hold
            ? new Promise<Response>((resolve) => (release = resolve))
            : jsonResponse(200, { games: [listed({ game: 'first' })] }),
      },
    })
    await screen.findByRole('table', TABLE)
    hold = true
    await app.user.click(refresh())
    await waitFor(() => expect(refresh()).toHaveAttribute('aria-disabled', 'true'))
    expect(screen.getByRole('region', { name: 'Your games' })).toHaveAttribute('aria-busy', 'true')
    expect(rows()).toHaveLength(1) // the old list stays up
    release(
      jsonResponse(200, {
        games: [
          listed({ game: 'second', created_at: ago(1) }),
          listed({ game: 'first', created_at: ago(30) }),
        ],
      }),
    )
    await waitFor(() => expect(rows()).toHaveLength(2))
    expect(refresh()).not.toHaveAttribute('aria-disabled', 'true')
    expect(ids()).toEqual(['second', 'first'])
  })
})

describe('each game', () => {
  const bo = { seat: 1, name: 'Bo', user: null }

  it('shows its players, state, age, the user’s own seat and a Continue link', async () => {
    await openRows([
      listed({
        game: 'g-running',
        status: 'running',
        created_at: ago(5),
        name: 'Ada',
        seat: 0,
        others: [
          { seat: 1, name: 'Player 2', user: 'bo' },
          { seat: 2, name: 'Cy', user: null },
        ],
      }),
    ])
    const game = within(rows()[0] as HTMLElement)
    expect(game.getByRole('rowheader')).toHaveTextContent('bo and Cy')
    expect(game.getByText('Running')).toBeInTheDocument()
    expect(game.getByText('5 minutes ago')).toHaveAttribute('datetime', ago(5))
    expect(game.getByText('Ada, seat 1')).toBeInTheDocument()
    const link = game.getByRole('link', { name: 'Continue your game with bo and Cy' })
    expect(link).toHaveTextContent('Continue')
    expect(link).toHaveAttribute('href', '/game/g-running')
  })

  it('titles every column of the table, in the words of the rows', async () => {
    await openRows([listed({ others: [bo] })])
    const heads = within(table()).getAllByRole('columnheader')
    expect(heads.map((head) => head.textContent)).toEqual([
      'Players',
      'State',
      'Result',
      'Your seat',
      'Started',
      'Action',
    ])
  })

  it.each([
    ['lobby', 'Lobby'],
    ['running', 'Running'],
  ] as const)(
    'offers Continue for a game in the %s state, as a link with no seat link or token, and no result',
    async (status, badge) => {
      await openRows([listed({ game: 'g-1', status, others: [bo] })])
      const row = within(rows()[0] as HTMLElement)
      const link = row.getByRole('link', { name: /continue/i })
      expect(link).toHaveAttribute('href', '/game/g-1')
      expect(link.getAttribute('href')).not.toMatch(/[#?]|token|seat=/)
      expect(row.getByText(badge)).toBeInTheDocument()
      expect(row.getByText('Not over')).toHaveClass('sr-only')
    },
  )

  it.each([
    ['won', { Winner: 0 } as const, 'You won'],
    ['lost to a guest', { Winner: 1 } as const, 'Bo won'],
    ['drawn', 'Draw' as const, 'Draw'],
    ['finished with no result on record', null, 'Ended without a result'],
  ])('shows a game that was %s as “%s” and offers no way back in', async (_how, result, words) => {
    await openRows([listed({ status: 'finished', result, others: [bo] })])
    const row = within(rows()[0] as HTMLElement)
    expect(row.getByText('Finished')).toBeInTheDocument()
    expect(row.getByText(words)).toBeInTheDocument()
    expect(row.queryByRole('link')).toBeNull()
    expect(row.queryByRole('button')).toBeNull()
  })

  it('writes an old game as a date', async () => {
    await openRows([listed({ status: 'finished', result: 'Draw', created_at: '2026-08-02T09:30:00Z' })])
    expect(within(rows()[0] as HTMLElement).getByText('on 2 Aug 2026')).toBeInTheDocument()
  })

  it('has one row per game and tells the games apart by their other players', async () => {
    await openRows([
      listed({ game: 'a', others: [{ seat: 1, name: 'Bo', user: null }] }),
      listed({ game: 'b', others: [{ seat: 1, name: 'Cy', user: 'cy' }] }),
    ])
    expect(rows()).toHaveLength(2)
    expect(screen.getByRole('link', { name: 'Continue your game with Bo' })).toHaveAttribute(
      'href',
      '/game/a',
    )
    expect(screen.getByRole('link', { name: 'Continue your game with cy' })).toHaveAttribute(
      'href',
      '/game/b',
    )
  })

  it('names all eleven other players of a twelve-seat game', async () => {
    const others = Array.from({ length: 11 }, (_, i) => ({
      seat: i + 1,
      name: `Player ${i + 2}`,
      user: null,
    }))
    await openRows([listed({ others })])
    const text = rows()[0]?.textContent ?? ''
    expect(text).toContain('Player 2, Player 3')
    expect(text).toContain('and Player 12')
  })
})

describe('on a narrow screen', () => {
  const bo = { seat: 1, name: 'Bo', user: null }
  const stacked = () => screen.getByRole('list', { name: 'Your games, newest first' })
  const items = () => within(stacked()).getAllByRole('listitem')

  async function openStacked(games: MyGame[]) {
    narrowWindow()
    const server = startAccounts({ users, signedIn: ADA.username, games })
    const store = createAuthStore({ api: server.api })
    await renderInRouter(
      <AuthProvider store={store}>
        <MyGamesPage clock={() => NOW} />
      </AuthProvider>,
    )
    await screen.findByRole('list', { name: 'Your games, newest first' })
  }

  it('stacks each game as a row with its badge, age, result and seat, and no table', async () => {
    await openStacked([
      listed({ game: 'won', status: 'finished', result: { Winner: 0 }, created_at: ago(5), others: [bo] }),
    ])
    expect(screen.queryByRole('table')).toBeNull()
    const row = within(items()[0] as HTMLElement)
    expect(items()[0]).toHaveAttribute('data-status', 'finished')
    expect(row.getByText('Finished')).toBeInTheDocument()
    expect(row.getByText('5 minutes ago')).toHaveAttribute('datetime', ago(5))
    expect(row.getByText('You won')).toBeInTheDocument()
    expect(row.getByText('Bo')).toBeInTheDocument()
    expect(row.getByText('You play Ada in seat 1.')).toBeInTheDocument()
    expect(row.queryByRole('link')).toBeNull()
  })

  it('offers Continue on a running game, beside its words', async () => {
    await openStacked([listed({ game: 'live', status: 'running', others: [bo] })])
    const link = within(items()[0] as HTMLElement).getByRole('link', { name: 'Continue your game with Bo' })
    expect(link).toHaveTextContent('Continue')
    expect(link).toHaveAttribute('href', '/game/live')
  })
})
