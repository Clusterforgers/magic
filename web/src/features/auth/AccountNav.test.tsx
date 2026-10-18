// Goal: the header offers exactly what the sign-in state allows (nothing while unknown or without accounts, "Sign in" and "Register" for a guest, the username, "My games" and "Sign out" for an account), signing out clears the account and the games list and leaves a page that needs one, and a sign-out the server did not get is reported, not faked.
import { screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { authError, listed } from '@/test-utils/accountsServer'
import type { AccountsScript } from '@/test-utils/accountsServer'
import { renderApp } from '@/test-utils/appHarness'

afterEach(() => {
  vi.unstubAllGlobals()
})

const ADA = { username: 'Ada_L', password: 'correct horse battery' }
const users = { [ADA.username]: ADA.password }

const nav = () => screen.queryByRole('navigation', { name: 'Account' })

async function open(url = '/', accounts: AccountsScript = {}) {
  const app = await renderApp(url, { accounts: { users, ...accounts } })
  await waitFor(() => expect(app.store.getState().status).not.toBe('loading'))
  return app
}

describe('while the server is being asked', () => {
  it('shows no account links, so a signed-in visitor never sees "Sign in" flash by', async () => {
    let answer!: (response: Response) => void
    const app = await renderApp('/', {
      accounts: {
        users,
        signedIn: ADA.username,
        override: { me: () => new Promise<Response>((r) => (answer = r)) },
      },
    })
    expect(app.store.getState().status).toBe('loading')
    expect(nav()).toBeNull()
    expect(screen.getByRole('link', { name: 'Magic' })).toHaveAttribute('href', '/')
    answer(new Response(JSON.stringify({ id: 'x', username: ADA.username }), { status: 200 }))
    expect(await screen.findByRole('navigation', { name: 'Account' })).toBeInTheDocument()
  })
})

describe('a visitor who is not signed in', () => {
  it('is offered "Sign in" and "Register", and nothing about games', async () => {
    await open()
    const links = within(nav()!).getAllByRole('link')
    expect(links.map((link) => link.textContent)).toEqual(['Sign in', 'Register'])
    expect(links.map((link) => link.getAttribute('href'))).toEqual(['/login', '/register'])
    expect(within(nav()!).queryByRole('button')).toBeNull()
    expect(screen.queryByRole('link', { name: 'My games' })).toBeNull()
  })

  it('reaches the forms through those links', async () => {
    const app = await open()
    await app.user.click(within(nav()!).getByRole('link', { name: 'Register' }))
    expect(await screen.findByRole('form', { name: 'Create an account' })).toBeInTheDocument()
    await app.user.click(within(nav()!).getByRole('link', { name: 'Sign in' }))
    expect(await screen.findByRole('form', { name: 'Sign in' })).toBeInTheDocument()
  })

  it('is also what a server that could not say who is signed in shows', async () => {
    await open('/', { signedIn: ADA.username, override: { me: () => authError(500, 'internal') } })
    expect(within(nav()!).getAllByRole('link')).toHaveLength(2)
  })
})

describe('a server without accounts', () => {
  it('offers no account links at all, and the create page works as it always did', async () => {
    const app = await open('/', { unavailable: true })
    expect(app.store.getState().status).toBe('unavailable')
    expect(nav()).toBeNull()
    expect(screen.queryByRole('link', { name: /sign in|register/i })).toBeNull()
    expect(await screen.findByRole('form', { name: 'Create a game' })).toBeInTheDocument()
  })
})

describe('a signed-in account', () => {
  it('shows the username, "My games" and "Sign out"', async () => {
    await open('/', { signedIn: ADA.username })
    const header = within(nav()!)
    expect(header.getByText(ADA.username)).toBeInTheDocument()
    expect(header.getByText('Signed in as', { exact: false })).toHaveClass('sr-only')
    expect(header.getByRole('link', { name: 'My games' })).toHaveAttribute('href', '/games')
    expect(header.getByRole('button', { name: 'Sign out' })).toBeEnabled()
    expect(header.queryByRole('link', { name: 'Sign in' })).toBeNull()
    expect(header.queryByRole('link', { name: 'Register' })).toBeNull()
  })

  it('reaches the list through "My games"', async () => {
    const app = await open('/', { signedIn: ADA.username, games: [listed()] })
    await app.user.click(within(nav()!).getByRole('link', { name: 'My games' }))
    expect(await screen.findByRole('heading', { name: 'My games' })).toBeInTheDocument()
    expect(app.router.state.location.pathname).toBe('/games')
  })

  it('shows a long username cut short, with the whole name available on hover', async () => {
    const long = 'a_very_long_username_of_32_chars_'.slice(0, 32)
    await open('/', { users: { [long]: 'x'.repeat(12) }, signedIn: long })
    const name = within(nav()!).getByText(long)
    expect(name).toHaveAttribute('title', long)
    expect(name).toHaveClass('truncate')
  })

  it('signs out: the server is told, the links go back to guest ones, and the user is forgotten', async () => {
    const app = await open('/', { signedIn: ADA.username })
    await app.user.click(within(nav()!).getByRole('button', { name: 'Sign out' }))
    await waitFor(() => expect(app.store.getState().status).toBe('signed-out'))
    expect(app.accounts.signedIn).toBeNull()
    expect(app.store.getState()).toMatchObject({ user: null, games: { status: 'idle', games: [] } })
    expect(
      within(nav()!)
        .getAllByRole('link')
        .map((link) => link.textContent),
    ).toEqual(['Sign in', 'Register'])
    expect(screen.queryByText(ADA.username)).toBeNull()
  })

  it('clears the games list of the account, so the next account never sees it', async () => {
    const bo = { username: 'Bo', password: 'bo has a long password' }
    const app = await open('/games', {
      users: { ...users, [bo.username]: bo.password },
      signedIn: ADA.username,
      games: [listed({ game: 'ada-game' })],
    })
    expect(await screen.findByRole('list', { name: 'Your games' })).toBeInTheDocument()
    await app.user.click(within(nav()!).getByRole('button', { name: 'Sign out' }))
    await waitFor(() => expect(app.store.getState().status).toBe('signed-out'))
    expect(app.store.getState().games.games).toEqual([])
    await app.store.getState().login(bo)
    expect(app.store.getState().games).toMatchObject({ status: 'idle', games: [] })
  })

  it('goes home when it signs out on a page that needs an account, not to the sign-in form', async () => {
    const app = await open('/games', { signedIn: ADA.username, games: [listed()] })
    await screen.findByRole('list', { name: 'Your games' })
    await app.user.click(within(nav()!).getByRole('button', { name: 'Sign out' }))
    await waitFor(() => expect(app.store.getState().status).toBe('signed-out'))
    await waitFor(() => expect(app.router.state.location.pathname).toBe('/'))
    expect(await screen.findByRole('form', { name: 'Create a game' })).toBeInTheDocument()
  })

  it('stays on other pages when it signs out', async () => {
    const app = await open('/register', { signedIn: ADA.username })
    // Signed in at the register page means the page has already moved on; sign out where we land.
    await waitFor(() => expect(app.router.state.location.pathname).toBe('/'))
    await app.user.click(within(nav()!).getByRole('button', { name: 'Sign out' }))
    await waitFor(() => expect(app.store.getState().status).toBe('signed-out'))
    expect(app.router.state.location.pathname).toBe('/')
  })

  it('says so, and stays signed in, when the server did not get the sign-out', async () => {
    const app = await open('/', {
      signedIn: ADA.username,
      override: { logout: () => Promise.reject(new TypeError('Failed to fetch')) },
    })
    await app.user.click(within(nav()!).getByRole('button', { name: 'Sign out' }))
    const alert = await screen.findByRole('alert')
    expect(alert).toHaveTextContent(/could not sign out.*could not reach the server/i)
    expect(app.store.getState().status).toBe('signed-in')
    expect(within(nav()!).getByText(ADA.username)).toBeInTheDocument()
    expect(within(nav()!).getByRole('button', { name: 'Sign out' })).toBeEnabled()
    await app.user.click(within(alert).getByRole('button', { name: 'Dismiss' }))
    expect(screen.queryByRole('alert')).toBeNull()
  })

  it('disables "Sign out" while the server is being told', async () => {
    let answer!: (response: Response) => void
    const app = await open('/', {
      signedIn: ADA.username,
      override: { logout: () => new Promise<Response>((resolve) => (answer = resolve)) },
    })
    await app.user.click(within(nav()!).getByRole('button', { name: 'Sign out' }))
    expect(within(nav()!).getByRole('button', { name: 'Sign out' })).toBeDisabled()
    answer(new Response(null, { status: 204 }))
    await waitFor(() => expect(app.store.getState().status).toBe('signed-out'))
  })
})
