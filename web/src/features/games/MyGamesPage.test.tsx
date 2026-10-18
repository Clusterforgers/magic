// Goal: the My games page sends a visitor who is not signed in to the sign-in form and back, lists the user's games newest first with state, other players, age and result, offers Rejoin only where a game can be rejoined and without any seat link, and handles an empty list, a failed load, a reload and a session that ended.
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

const list = () => screen.getByRole('list', { name: 'Your games' })
const rows = () => within(list()).getAllByRole('listitem')

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
  await screen.findByRole('list', { name: 'Your games' })
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
    expect(await screen.findByRole('list', { name: 'Your games' })).toBeInTheDocument()
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
    expect(screen.queryByRole('list')).toBeNull()
  })
})

describe('a server without accounts', () => {
  it('explains that there is no list and points at creating a game', async () => {
    const app = await openApp({ unavailable: true })
    expect(
      await screen.findByText(/this server has no accounts, so it keeps no list of games/i),
    ).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Create a game' })).toHaveAttribute('href', '/')
    expect(app.accounts.calls.map((call) => call.url)).toEqual(['/api/auth/me'])
    expect(screen.queryByRole('navigation', { name: 'Account' })).toBeNull()
  })
})

describe('the list', () => {
  it('is titled, and asks the server for the games once the account is known', async () => {
    const app = await openApp({ games: [listed()] })
    expect(await screen.findByRole('heading', { level: 1, name: 'My games' })).toBeInTheDocument()
    expect(document.title).toBe('My games · Magic')
    await screen.findByRole('list', { name: 'Your games' })
    expect(app.accounts.calls.filter((call) => call.url === '/api/me/games')).toHaveLength(1)
    expect(app.accounts.calls.find((call) => call.url === '/api/me/games')?.init?.credentials).toBe('include')
  })

  it('says it is loading while the first answer is awaited', async () => {
    const app = await openApp({ override: { games: () => new Promise<Response>(() => undefined) } })
    await waitFor(() => expect(app.store.getState().games.status).toBe('loading'))
    expect(screen.getByRole('status')).toHaveTextContent('Loading your games…')
    expect(screen.getByRole('button', { name: 'Reloading…' })).toBeDisabled()
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
    await screen.findByRole('list', { name: 'Your games' })
    expect(rows().map((row) => row.getAttribute('data-game'))).toEqual(['fresh', 'mid', 'old'])
  })

  it('shows the empty state with a way to create a game', async () => {
    await openApp({ games: [] })
    expect(await screen.findByRole('heading', { name: 'You have no games yet' })).toBeInTheDocument()
    expect(screen.queryByRole('list', { name: 'Your games' })).toBeNull()
    const create = screen.getByRole('link', { name: 'Create a game' })
    expect(create).toHaveAttribute('href', '/')
    expect(screen.queryByRole('status')).toBeNull()
  })

  it('opens the create page from the empty state', async () => {
    const app = await openApp({ games: [] })
    await app.user.click(await screen.findByRole('link', { name: 'Create a game' }))
    expect(await screen.findByRole('form', { name: 'Create a game' })).toBeInTheDocument()
  })

  it('asks for the games at most twice when React mounts everything twice', async () => {
    const app = await openApp({ games: [listed()] }, true)
    await screen.findByRole('list', { name: 'Your games' })
    expect(app.accounts.calls.filter((call) => call.url === '/api/me/games').length).toBeLessThanOrEqual(2)
    expect(rows()).toHaveLength(1)
  })
})

describe('when loading fails', () => {
  it('says why in words, offers a retry, and the retry works', async () => {
    let broken = true
    const app = await openApp({
      games: [listed()],
      override: {
        games: () => (broken ? authError(503, 'unavailable') : jsonResponse(200, { games: [listed()] })),
      },
    })
    const alert = await screen.findByRole('alert')
    expect(alert).toHaveTextContent('Could not load your games')
    expect(alert).toHaveTextContent(/cannot check accounts right now/i)
    expect(screen.queryByRole('list', { name: 'Your games' })).toBeNull()
    broken = false
    await app.user.click(within(alert).getByRole('button', { name: 'Try again' }))
    expect(await screen.findByRole('list', { name: 'Your games' })).toBeInTheDocument()
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

  it('keeps the list it had and says the reload failed', async () => {
    let broken = false
    const app = await openApp({
      games: [listed({ game: 'kept' })],
      override: {
        games: () =>
          broken ? authError(500, 'internal') : jsonResponse(200, { games: [listed({ game: 'kept' })] }),
      },
    })
    await screen.findByRole('list', { name: 'Your games' })
    broken = true
    await app.user.click(screen.getByRole('button', { name: 'Reload' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('Could not load your games')
    expect(rows().map((row) => row.getAttribute('data-game'))).toEqual(['kept'])
  })

  it('sends the visitor to the sign-in form when the session turns out to have ended', async () => {
    const app = await openApp({ games: [listed()] })
    await screen.findByRole('list', { name: 'Your games' })
    await app.accounts.api.logout() // the cookie is gone: expired, or signed out in another tab
    await app.user.click(screen.getByRole('button', { name: 'Reload' }))
    expect(await screen.findByRole('form', { name: 'Sign in' })).toBeInTheDocument()
    expect(app.router.state.matches.at(-1)?.search).toEqual({ next: '/games' })
    expect(app.store.getState()).toMatchObject({ status: 'signed-out', user: null })
  })
})

describe('reloading', () => {
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
    await screen.findByRole('list', { name: 'Your games' })
    hold = true
    await app.user.click(screen.getByRole('button', { name: 'Reload' }))
    const busy = await screen.findByRole('button', { name: 'Reloading…' })
    expect(busy).toBeDisabled()
    expect(list()).toHaveAttribute('aria-busy', 'true')
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
    expect(screen.getByRole('button', { name: 'Reload' })).toBeEnabled()
    expect(rows().map((row) => row.getAttribute('data-game'))).toEqual(['second', 'first'])
  })
})

describe('each game', () => {
  const bo = { seat: 1, name: 'Bo', user: null }

  it('shows its state, its age, the other players, the user’s own seat and a Rejoin link', async () => {
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
    const [row] = rows()
    const game = within(row!)
    expect(game.getByText('Running')).toBeInTheDocument()
    expect(game.getByText('5 minutes ago')).toHaveAttribute('datetime', ago(5))
    expect(game.getByText('bo and Cy')).toBeInTheDocument()
    expect(game.getByText('You play Ada in seat 1.')).toBeInTheDocument()
    expect(game.getByRole('link', { name: 'Rejoin the game with bo and Cy' })).toHaveAttribute(
      'href',
      '/game/g-running',
    )
  })

  it.each([
    ['lobby', 'Lobby'],
    ['running', 'Running'],
  ] as const)(
    'offers Rejoin for a game in the %s state, as a link with no seat link or token',
    async (status, badge) => {
      await openRows([listed({ game: 'g-1', status, others: [bo] })])
      const link = within(rows()[0]!).getByRole('link', { name: /rejoin/i })
      expect(link).toHaveAttribute('href', '/game/g-1')
      expect(link.getAttribute('href')).not.toMatch(/[#?]|token|seat=/)
      expect(within(rows()[0]!).getByText(badge)).toBeInTheDocument()
    },
  )

  it.each([
    ['won', { Winner: 0 } as const, 'You won'],
    ['lost to a guest', { Winner: 1 } as const, 'Bo won'],
    ['drawn', 'Draw' as const, 'Draw'],
  ])('shows a finished game that was %s as “%s” and offers no way back in', async (_how, result, words) => {
    await openRows([listed({ status: 'finished', result, others: [bo] })])
    const row = within(rows()[0]!)
    expect(row.getByText('Finished')).toBeInTheDocument()
    expect(row.getByText(words)).toBeInTheDocument()
    expect(row.queryByRole('link')).toBeNull()
    expect(row.queryByRole('button')).toBeNull()
  })

  it('writes an old game as a date', async () => {
    await openRows([listed({ status: 'finished', result: 'Draw', created_at: '2026-08-02T09:30:00Z' })])
    expect(within(rows()[0]!).getByText('on 2 Aug 2026')).toBeInTheDocument()
  })

  it('has one row per game and tells the games apart by their other players', async () => {
    await openRows([
      listed({ game: 'a', others: [{ seat: 1, name: 'Bo', user: null }] }),
      listed({ game: 'b', others: [{ seat: 1, name: 'Cy', user: 'cy' }] }),
    ])
    expect(rows()).toHaveLength(2)
    expect(screen.getByRole('link', { name: 'Rejoin the game with Bo' })).toHaveAttribute('href', '/game/a')
    expect(screen.getByRole('link', { name: 'Rejoin the game with cy' })).toHaveAttribute('href', '/game/b')
  })

  it('names all eleven other players of a twelve-seat game', async () => {
    const others = Array.from({ length: 11 }, (_, i) => ({
      seat: i + 1,
      name: `Player ${i + 2}`,
      user: null,
    }))
    await openRows([listed({ others })])
    const text = rows()[0]!.textContent ?? ''
    expect(text).toContain('Player 2, Player 3')
    expect(text).toContain('and Player 12')
  })
})
