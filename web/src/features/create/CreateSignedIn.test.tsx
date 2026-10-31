// Goal: a signed-in creator is told they play seat 1 as themselves and that the game lands in My games, the request carries their session, and a guest's or a no-accounts server's create page stays plain.
import { screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { authError, listed } from '@/test-utils/accountsServer'
import type { AccountsScript } from '@/test-utils/accountsServer'
import { renderApp } from '@/test-utils/appHarness'
import type { App } from '@/test-utils/appHarness'
import { chooseSeatLinks } from '@/test-utils/createForm'
import { MADE_GAME, tokenOf } from '@/test-utils/fakeServer'

afterEach(() => {
  vi.unstubAllGlobals()
})

const ADA = { username: 'Ada_L', password: 'correct horse battery' }
const users = { [ADA.username]: ADA.password }

const note = () => document.querySelector('[data-slot="signed-in-note"]')
const seatName = () => screen.getByLabelText('Your name')
const myGamesRequests = (app: App) => app.accounts.calls.filter((call) => call.url === '/api/me/games')
const create = () => screen.getByRole('button', { name: 'Create game' })

async function openCreate(accounts: AccountsScript = {}) {
  const app = await renderApp('/new', { accounts: { users, ...accounts } })
  await screen.findByRole('form', { name: 'Create a game' })
  await app.user.click(screen.getByRole('radio', { name: /^Normal/ }))
  return app
}

describe('a signed-in creator', () => {
  it('is told who they are and that they play seat 1, and that the game lands in My games', async () => {
    await openCreate({ signedIn: ADA.username })
    expect(note()).toHaveTextContent(
      'Signed in as Ada_L. You will play as Ada_L in seat 1. The game will be listed under My games.',
    )
    expect(within(note() as HTMLElement).getByRole('link', { name: 'My games' })).toHaveAttribute(
      'href',
      '/games',
    )
  })

  it('adds that the other links are for friends once every seat gets a link', async () => {
    const app = await openCreate({ signedIn: ADA.username })
    await chooseSeatLinks(app.user)
    expect(note()).toHaveTextContent(
      'Signed in as Ada_L. You will play as Ada_L in seat 1; the other links are for your friends. The game will be listed under My games.',
    )
  })

  it('has the creator’s name set to the account and no fields for the open seats', async () => {
    await openCreate({ signedIn: ADA.username })
    expect(seatName()).toHaveValue('Ada_L')
    expect(screen.queryByLabelText('Name for seat 2')).toBeNull()
  })

  it('follows the name given to seat 1, and falls back to the account when it is emptied', async () => {
    const app = await openCreate({ signedIn: ADA.username })
    await app.user.clear(seatName())
    expect(note()).toHaveTextContent('You will play as Ada_L in seat 1')
    await app.user.type(seatName(), 'The Lovelace')
    expect(note()).toHaveTextContent('You will play as The Lovelace in seat 1')
    expect(note()).toHaveTextContent('Signed in as Ada_L.')
  })

  it('waits for the server to say who is signed in before it shows the form', async () => {
    let answer!: (response: Response) => void
    const app = await renderApp('/new', {
      accounts: {
        users,
        signedIn: ADA.username,
        override: { me: () => new Promise<Response>((r) => (answer = r)) },
      },
    })
    await screen.findByText('Loading the decks…')
    await waitFor(() => expect(app.rest.count('/api/demo-decks')).toBeGreaterThan(0))
    await app.settle()
    expect(screen.queryByRole('form', { name: 'Create a game' })).toBeNull()
    answer(
      new Response(JSON.stringify({ id: 'x', username: ADA.username, is_admin: false }), { status: 200 }),
    )
    expect(await screen.findByRole('form', { name: 'Create a game' })).toBeInTheDocument()
    expect(seatName()).toHaveValue('Ada_L')
  })

  it('creates the game with the session cookie, the account’s seat first and held alone, and opens its lobby', async () => {
    const app = await openCreate({ signedIn: ADA.username, games: [] })
    app.accounts.setGames([listed({ game: MADE_GAME, name: 'Ada_L', status: 'lobby' })])
    await app.user.click(create())
    await waitFor(() =>
      expect(app.router.state.location.href).toBe(`/game/${MADE_GAME}#seat=0&token=${tokenOf(0)}`),
    )
    expect(app.rest.creates).toEqual([
      {
        format: 'normal',
        seats: [{ name: 'Ada_L', deck: 'mono-green' }, { name: 'Player 2' }],
        visibility: 'public',
        held: 1,
      },
    ])
    const request = app.rest.calls.find((call) => call.url === '/api/games' && call.init?.method === 'POST')
    expect(request?.init?.credentials).toBe('include')
  })

  it('creates a game with a link for every seat and shows where to find it', async () => {
    const app = await openCreate({ signedIn: ADA.username, games: [] })
    app.accounts.setGames([listed({ game: MADE_GAME, name: 'Ada_L', status: 'lobby' })])
    await chooseSeatLinks(app.user)
    await app.user.click(create())
    expect(await screen.findByRole('heading', { name: 'Game created' })).toBeInTheDocument()
    expect(app.rest.creates).toEqual([
      {
        format: 'normal',
        seats: [
          { name: 'Ada_L', deck: 'mono-green' },
          { name: 'Player 2', deck: 'mono-red' },
        ],
        visibility: 'invite',
      },
    ])
    const saved = document.querySelector('[data-slot="saved-note"]')
    expect(saved).toHaveTextContent('Seat 1 (Ada_L) is yours and saved to your account, Ada_L.')
    expect(saved).toHaveTextContent('rejoin it from there without a link')
    expect(within(saved as HTMLElement).getByRole('link', { name: 'My games' })).toHaveAttribute(
      'href',
      '/games',
    )
  })

  it('keeps the seat links flow as it was: a link per seat, copy all, and open my seat', async () => {
    const app = await openCreate({ signedIn: ADA.username })
    await chooseSeatLinks(app.user)
    await app.user.click(create())
    await screen.findByRole('heading', { name: 'Game created' })
    expect(within(screen.getByRole('list', { name: 'Seat links' })).getAllByRole('listitem')).toHaveLength(2)
    expect(screen.getByRole('button', { name: 'Open my seat (Ada_L)' })).toBeEnabled()
    expect(screen.getByRole('button', { name: 'Copy all links' })).toBeEnabled()
  })

  it('refreshes the list of games after creating, so My games has it', async () => {
    const app = await openCreate({ signedIn: ADA.username, games: [] })
    const before = myGamesRequests(app).length
    await app.user.click(create())
    await waitFor(() => expect(myGamesRequests(app).length).toBe(before + 1))
  })

  it('stops saying the game is saved when the session turns out to have ended', async () => {
    const app = await openCreate({
      signedIn: ADA.username,
      override: { games: () => authError(401, 'unauthenticated') },
    })
    await chooseSeatLinks(app.user)
    await app.user.click(create())
    await screen.findByRole('heading', { name: 'Game created' })
    await waitFor(() => expect(app.store.getState().status).toBe('signed-out'))
    expect(document.querySelector('[data-slot="saved-note"]')).toBeNull()
    expect(screen.getByRole('link', { name: 'Sign in' })).toBeInTheDocument()
  })

  it('loses the sentence when the account signs out', async () => {
    const app = await openCreate({ signedIn: ADA.username })
    await app.user.click(
      within(screen.getByRole('navigation', { name: 'Account' })).getByRole('button', {
        name: /signed in as/i,
      }),
    )
    await app.user.click(await screen.findByRole('menuitem', { name: 'Sign out' }))
    await waitFor(() => expect(note()).toBeNull())
    expect(seatName()).toHaveValue('Ada_L') // what was typed stays
  })
})

describe('a guest', () => {
  it('sees the page without a sentence about an account, the creator named Player 1', async () => {
    await openCreate()
    expect(note()).toBeNull()
    expect(seatName()).toHaveValue('Player 1')
  })

  it('creates a game with a link for every seat with no saved-to-account note and asks nothing of the games list', async () => {
    const app = await openCreate()
    await chooseSeatLinks(app.user)
    await app.user.click(create())
    await screen.findByRole('heading', { name: 'Game created' })
    expect(document.querySelector('[data-slot="saved-note"]')).toBeNull()
    expect(myGamesRequests(app)).toHaveLength(0)
    expect(screen.getByRole('button', { name: 'Open my seat (Player 1)' })).toBeEnabled()
  })

  it('creates the simple game and asks nothing of the games list', async () => {
    const app = await openCreate()
    await app.user.click(create())
    await waitFor(() => expect(app.router.state.location.pathname).toBe(`/game/${MADE_GAME}`))
    expect(myGamesRequests(app)).toHaveLength(0)
  })
})

describe('a server without accounts', () => {
  it('shows the page without a sentence, after one look at the account endpoint', async () => {
    const app = await openCreate({ unavailable: true })
    expect(note()).toBeNull()
    expect(seatName()).toHaveValue('Player 1')
    expect(app.accounts.calls.map((call) => call.url)).toEqual(['/api/auth/options', '/api/auth/me'])
  })
})
