// Goal: a game page joins by seat link when the address has one and through the signed-in session when it has not, keeps the same credential on every reconnect, tells everybody else what to do instead of showing a blank page, and never puts a token anywhere the session does the work.
import { screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { PROTOCOL_VERSION } from '@/protocol/generated'
import { listed, startAccounts } from '@/test-utils/accountsServer'
import type { AccountsScript } from '@/test-utils/accountsServer'
import { renderApp } from '@/test-utils/appHarness'
import { tokenOf } from '@/test-utils/fakeServer'
import { GAME_ID, renderGame } from '@/test-utils/gameHarness'
import { createAuthStore } from '@/features/auth/store'
import { UNREACHABLE_AFTER } from './connection'

afterEach(() => {
  vi.unstubAllGlobals()
  window.sessionStorage.clear()
})

const ADA = { username: 'Ada_L', password: 'correct horse battery' }
const users = { [ADA.username]: ADA.password }
const SESSION_URL = `ws://localhost:5173/ws/${GAME_ID}`
const alert = () => screen.getByRole('alert')

describe('a signed-in account on an address without a seat link', () => {
  it('opens one socket at the game address with no token, and joins under the account name', async () => {
    const h = await renderGame({ link: null, account: ADA.username })
    expect(h.net.sockets).toHaveLength(1)
    expect(h.socket().url).toBe(SESSION_URL)
    h.socket().open()
    expect(h.socket().sent).toEqual([
      { type: 'Join', protocol_version: PROTOCOL_VERSION, game: GAME_ID, name: ADA.username },
    ])
    expect(screen.getByRole('heading', { name: 'Connecting to the game…' })).toBeInTheDocument()
  })

  it('shows the lobby for the seat the server says the session opened, not one the page named', async () => {
    const h = await renderGame({ link: null, account: ADA.username })
    h.join(2, 3)
    expect(screen.getByRole('heading', { name: 'Waiting for players' })).toBeInTheDocument()
    const rows = within(screen.getByRole('list', { name: 'Seats' })).getAllByRole('listitem')
    expect(rows).toHaveLength(3)
    expect(rows[2]).toHaveTextContent('you')
    expect(rows[0]).not.toHaveTextContent('you')
    expect(document.title).toBe('Lobby · Magic')
  })

  it('opens one socket when React mounts everything twice', async () => {
    const h = await renderGame({ link: null, account: ADA.username, strict: true })
    expect(h.net.sockets).toHaveLength(1)
    expect(h.socket().closedByClient).toBeNull()
  })

  it('keeps using the session address for every retry after a lost connection', async () => {
    const h = await renderGame({ link: null, account: ADA.username })
    h.join(0, 2)
    h.fail()
    await h.advance(250)
    h.fail()
    await h.advance(15_000)
    expect(h.net.sockets.map((socket) => socket.url)).toEqual([SESSION_URL, SESSION_URL, SESSION_URL])
    h.join(0, 2)
    expect(screen.getByRole('heading', { name: 'Waiting for players' })).toBeInTheDocument()
    // A connection that opened says who it is the same way as the first: a Join under the account name.
    expect(h.net.sockets[0]?.sent[0]).toMatchObject({ type: 'Join', name: ADA.username })
    expect(h.net.sockets[2]?.sent[0]).toMatchObject({ type: 'Join', name: ADA.username })
  })

  it('takes the seat back through the session after another window replaced it', async () => {
    const h = await renderGame({ link: null, account: ADA.username })
    h.join(0, 2)
    h.close(4000)
    await h.user.click(screen.getByRole('button', { name: 'Use this window instead' }))
    expect(h.net.sockets.map((socket) => socket.url)).toEqual([SESSION_URL, SESSION_URL])
  })

  it('names the ways a session can fail to reach a game, and offers My games', async () => {
    const h = await renderGame({ link: null, account: ADA.username })
    for (let attempt = 1; attempt <= UNREACHABLE_AFTER; attempt += 1) {
      h.fail()
      if (attempt < UNREACHABLE_AFTER) await h.advance(15_000)
    }
    expect(alert()).toHaveTextContent('Cannot reach the game')
    expect(alert()).toHaveTextContent(/you may have been signed out/i)
    expect(alert()).toHaveTextContent(/no seat in this game is saved to your account/i)
    expect(alert()).not.toHaveTextContent(/seat link may be wrong/i)
    expect(within(alert()).getByRole('link', { name: 'My games' })).toHaveAttribute('href', '/games')
    expect(within(alert()).getByRole('link', { name: 'Create a new game' })).toHaveAttribute('href', '/new')
  })

  it('adds that the game is gone when the server says it does not exist', async () => {
    const h = await renderGame({ link: null, account: ADA.username })
    for (let attempt = 1; attempt <= UNREACHABLE_AFTER; attempt += 1) {
      h.fail()
      if (attempt < UNREACHABLE_AFTER) await h.advance(15_000)
    }
    expect(alert()).toHaveTextContent(/server also says this game does not exist/i)
  })

  it('suggests opening the seat link once while signed in when the game exists but the seat is not saved', async () => {
    const info = { started: false, seats: [{ seat: 0, name: 'Ana' }], connected: [], result: null }
    const h = await renderGame({
      link: null,
      account: ADA.username,
      script: { game: () => Promise.resolve(new Response(JSON.stringify(info), { status: 200 })) },
    })
    for (let attempt = 1; attempt <= UNREACHABLE_AFTER; attempt += 1) {
      h.fail()
      if (attempt < UNREACHABLE_AFTER) await h.advance(15_000)
    }
    expect(alert()).toHaveTextContent(/open it once while signed in and the seat is saved/i)
    expect(alert()).not.toHaveTextContent(/does not exist/i)
  })

  it('prefers a seat link to the session when the address has both', async () => {
    const h = await renderGame({ link: { seat: 1, token: tokenOf(1) }, account: ADA.username })
    expect(h.socket().url).toBe(`${SESSION_URL}/${tokenOf(1)}`)
    h.socket().open()
    expect(h.socket().sent).toMatchObject([{ type: 'Join', name: 'Seat 2' }])
  })
})

describe('a guest on an address without a seat link', () => {
  it('opens no socket and asks for the seat link', async () => {
    const h = await renderGame({ link: null })
    expect(screen.getByRole('heading', { name: 'Open your seat link' })).toBeInTheDocument()
    expect(screen.queryByRole('link', { name: 'Sign in' })).toBeNull()
    expect(h.net.sockets).toHaveLength(0)
  })

  it('is told they can also sign in, when the server has accounts', async () => {
    const accounts = startAccounts({ users })
    const auth = createAuthStore({ api: accounts.api, initial: { status: 'signed-out' } })
    const h = await renderGame({ link: null, auth })
    expect(screen.getByRole('heading', { name: 'Open your seat link or sign in' })).toBeInTheDocument()
    expect(screen.getByText(/sign in instead: your seat is saved to the account/i)).toBeInTheDocument()
    expect(document.title).toBe('Open your seat link or sign in · Magic')
    const signIn = screen.getByRole('link', { name: 'Sign in' })
    expect(signIn.getAttribute('href')).toMatch(new RegExp(`^/login\\?next=(%2F|/)game(%2F|/)${GAME_ID}$`))
    expect(screen.getByRole('link', { name: 'Create a new game' })).toHaveAttribute('href', '/new')
    expect(h.net.sockets).toHaveLength(0)
  })

  it('is not offered a sign-in the server cannot honour', async () => {
    const accounts = startAccounts({ unavailable: true })
    const auth = createAuthStore({ api: accounts.api, initial: { status: 'unavailable' } })
    await renderGame({ link: null, auth })
    expect(screen.getByRole('heading', { name: 'Open your seat link' })).toBeInTheDocument()
    expect(screen.queryByRole('link', { name: 'Sign in' })).toBeNull()
  })
})

describe('the game route', () => {
  const GAME = '8f2c1a52-31de-4c77-9a35-0d1d3f6a7b10'
  const SOCKET = `ws://localhost:3000/ws/${GAME}`

  async function open(url: string, accounts: AccountsScript = {}) {
    return renderApp(url, { accounts: { users, ...accounts } })
  }

  it('joins through the session when the visitor is signed in and the address has no seat link', async () => {
    const app = await open(`/game/${GAME}`, { signedIn: ADA.username })
    expect(await screen.findByRole('heading', { name: 'Connecting to the game…' })).toBeInTheDocument()
    expect(app.sockets.map((socket) => socket.url)).toEqual([SOCKET])
  })

  it('joins with the token from the fragment, as a guest does, when the address has a seat link', async () => {
    const app = await open(`/game/${GAME}#seat=1&token=${tokenOf(1)}`, { signedIn: ADA.username })
    await screen.findByRole('heading', { name: 'Connecting to the game…' })
    expect(app.sockets.map((socket) => socket.url)).toEqual([`${SOCKET}/${tokenOf(1)}`])
    expect(document.body.innerHTML).not.toContain(tokenOf(1))
  })

  it('does not wait for the account when the address has a seat link', async () => {
    const app = await open(`/game/${GAME}#seat=0&token=${tokenOf(0)}`, {
      override: { me: () => new Promise<Response>(() => undefined) },
    })
    await screen.findByRole('heading', { name: 'Connecting to the game…' })
    expect(app.store.getState().status).toBe('loading')
    expect(app.sockets).toHaveLength(1)
  })

  it('says it is checking, and opens nothing, while it is not yet known who is signed in', async () => {
    const app = await open(`/game/${GAME}`, {
      signedIn: ADA.username,
      override: { me: () => new Promise<Response>(() => undefined) },
    })
    expect(screen.getByRole('status')).toHaveTextContent('Checking your sign-in…')
    expect(document.title).toBe('Checking your sign-in · Magic')
    expect(app.sockets).toHaveLength(0)
  })

  it('tells a visitor who is not signed in to open the seat link or sign in', async () => {
    const app = await open(`/game/${GAME}`)
    expect(await screen.findByRole('heading', { name: 'Open your seat link or sign in' })).toBeInTheDocument()
    expect(app.sockets).toHaveLength(0)
  })

  it('tells a visitor to a server without accounts to open the seat link', async () => {
    const app = await open(`/game/${GAME}`, { unavailable: true })
    expect(await screen.findByRole('heading', { name: 'Open your seat link' })).toBeInTheDocument()
    expect(screen.queryByRole('link', { name: 'Sign in' })).toBeNull()
    expect(app.sockets).toHaveLength(0)
  })

  it('treats a broken fragment like none: the session is used when signed in', async () => {
    const app = await open(`/game/${GAME}#seat=x&token=`, { signedIn: ADA.username })
    await screen.findByRole('heading', { name: 'Connecting to the game…' })
    expect(app.sockets.map((socket) => socket.url)).toEqual([SOCKET])
  })

  it('comes back to the game through the session after the visitor signs in from the explanation', async () => {
    const app = await open(`/game/${GAME}`)
    // The header has a "Sign in" link too; this is the one in the explanation.
    await app.user.click(within(await screen.findByRole('main')).getByRole('link', { name: 'Sign in' }))
    await screen.findByRole('form', { name: 'Sign in' })
    expect(app.router.state.matches.at(-1)?.search).toEqual({ next: `/game/${GAME}` })
    await app.user.type(screen.getByLabelText('Username'), ADA.username)
    await app.user.type(screen.getByLabelText('Password'), ADA.password)
    await app.user.click(screen.getByRole('button', { name: 'Sign in' }))
    await waitFor(() => expect(app.router.state.location.pathname).toBe(`/game/${GAME}`))
    expect(await screen.findByRole('heading', { name: 'Connecting to the game…' })).toBeInTheDocument()
    expect(app.sockets.map((socket) => socket.url)).toEqual([SOCKET])
  })

  it('lets go of the session connection, and says what to do, when the account signs out', async () => {
    const app = await open(`/game/${GAME}`, { signedIn: ADA.username })
    await screen.findByRole('heading', { name: 'Connecting to the game…' })
    await app.user.click(
      within(screen.getByRole('navigation', { name: 'Account' })).getByRole('button', {
        name: /signed in as/i,
      }),
    )
    await app.user.click(await screen.findByRole('menuitem', { name: 'Sign out' }))
    expect(await screen.findByRole('heading', { name: 'Open your seat link or sign in' })).toBeInTheDocument()
    expect(app.sockets[0]?.closed).toEqual({ code: 1000, reason: 'client closing' })
    expect(app.sockets).toHaveLength(1)
  })

  it('is what Rejoin in My games opens: the game address with no fragment, connected through the session', async () => {
    const app = await open('/games', {
      signedIn: ADA.username,
      games: [listed({ game: GAME, status: 'running' })],
    })
    await app.user.click(await screen.findByRole('link', { name: /rejoin/i }))
    await waitFor(() => expect(app.router.state.location.pathname).toBe(`/game/${GAME}`))
    expect(app.router.state.location.hash).toBe('')
    expect(app.router.state.location.href).toBe(`/game/${GAME}`)
    expect(await screen.findByRole('heading', { name: 'Connecting to the game…' })).toBeInTheDocument()
    expect(app.sockets.map((socket) => socket.url)).toEqual([SOCKET])
    expect(document.body.innerHTML).not.toMatch(/token/i)
  })
})
