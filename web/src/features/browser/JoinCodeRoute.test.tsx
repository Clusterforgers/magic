// Goal: the real route tree serves /join/$code inside the app shell with the code from the address in its normal form, joins through POST /api/join as a guest with the name given on the page or as the signed-in account with the session and no name, opens the game at the seat link the answer names (the bare page for an account's seat), asks the server nothing for a code that cannot be one, ends a refused code in one sentence whose links lead to Play and to the create page, and shows the wait from Retry-After and a lost connection on the form beside the shell's offline notice.
import { act, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { renderApp } from '@/test-utils/appHarness'
import type { App } from '@/test-utils/appHarness'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { MADE_GAME, tokenOf } from '@/test-utils/fakeServer'
import type { ServerScript } from '@/test-utils/fakeServer'

// A guest's name is kept for the next visit, so each test starts from a browser that has none.
beforeEach(() => localStorage.clear())
afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
  localStorage.clear()
})

const ADA = { username: 'Ada_L', password: 'correct horse battery' }

interface Visit {
  signedIn?: boolean
  join?: ServerScript['join']
}

function visit(path: string, { signedIn = false, join }: Visit = {}): Promise<App> {
  return renderApp(path, {
    accounts: signedIn ? { users: { [ADA.username]: ADA.password }, signedIn: ADA.username } : {},
    rest: join === undefined ? {} : { join },
  })
}

const here = (app: App) => app.router.state.location.href
const joinButton = () => screen.getByRole('button', { name: 'Join game' })
const inPage = () => within(screen.getByRole('main'))
const refused =
  (status: number, error: string, headers: Record<string, string> = {}) =>
  () =>
    new Response(JSON.stringify({ error, message: 'refused' }), {
      status,
      headers: { 'content-type': 'application/json', ...headers },
    })

/** A guest types a name on the page and presses Join. */
async function guestJoins(app: App, name = 'Ivy') {
  await app.user.type(await screen.findByLabelText('Your name'), name)
  await app.user.click(joinButton())
}

describe('/join/$code', () => {
  it('is the join page inside the app shell, with the code of the address in its normal form', async () => {
    const app = await visit('/join/k7m2-9qxa')
    expect(await screen.findByRole('heading', { level: 1, name: 'Join a game' })).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Magic' })).toHaveAttribute('href', '/')
    expect(screen.getByText('K7M2-9QXA')).toBeInTheDocument()
    expect(document.title).toBe('Join a game · Magic')
    expect(app.rest.joins).toEqual([])
    expect(app.sockets).toHaveLength(0)
  })

  it('reads the address a lobby hands out, the code without its hyphen', async () => {
    await visit('/join/K7M29QXA')
    expect(await screen.findByText('K7M2-9QXA')).toBeInTheDocument()
  })

  it('lets a guest give a name on the page and joins with it, opening the game at the seat link', async () => {
    const app = await visit('/join/k7m2-9qxa')
    const name = await screen.findByLabelText('Your name')
    await waitFor(() => expect(name).toHaveFocus())
    await guestJoins(app)
    expect(await screen.findByRole('heading', { name: 'Connecting to the game…' })).toBeInTheDocument()
    expect(app.rest.joins).toEqual([{ code: 'K7M29QXA', name: 'Ivy' }])
    expect(here(app)).toBe(`/game/${MADE_GAME}#seat=1&token=${tokenOf(1)}`)
    expect(app.sockets.map((socket) => socket.url)).toEqual([
      `ws://localhost:3000/ws/${MADE_GAME}/${tokenOf(1)}`,
    ])
    expect(document.body.innerHTML).not.toContain(tokenOf(1))
  })

  it('joins a signed-in account in one click with the session and no name', async () => {
    const app = await visit('/join/k7m2-9qxa', { signedIn: true })
    const button = await screen.findByRole('button', { name: 'Join game' })
    await waitFor(() => expect(button).toHaveFocus())
    expect(screen.getByText('You join as Ada_L.')).toBeInTheDocument()
    expect(screen.queryByLabelText('Your name')).not.toBeInTheDocument()
    await app.user.click(button)
    expect(await screen.findByRole('heading', { name: 'Connecting to the game…' })).toBeInTheDocument()
    expect(app.rest.joins).toEqual([{ code: 'K7M29QXA' }])
    expect(app.rest.calls.find((call) => call.url === '/api/join')?.init?.credentials).toBe('include')
    expect(here(app)).toBe(`/game/${MADE_GAME}#seat=1&token=${tokenOf(1)}`)
  })

  it('opens the bare game page when the account already held a seat and the answer has no token', async () => {
    const app = await visit('/join/k7m2-9qxa', {
      signedIn: true,
      join: () => jsonResponse(200, { game: MADE_GAME, seat: 0, token: null }),
    })
    await app.user.click(await screen.findByRole('button', { name: 'Join game' }))
    await waitFor(() => expect(here(app)).toBe(`/game/${MADE_GAME}`))
    await waitFor(() =>
      expect(app.sockets.map((socket) => socket.url)).toEqual([`ws://localhost:3000/ws/${MADE_GAME}`]),
    )
  })

  it('says a code that cannot be one without asking the server anything', async () => {
    const app = await visit('/join/not-a-code')
    expect(await screen.findByRole('alert')).toHaveTextContent('That is not a join code')
    expect(screen.queryByRole('button', { name: 'Join game' })).not.toBeInTheDocument()
    expect(app.rest.calls.filter((call) => call.url === '/api/join')).toEqual([])
    expect(inPage().getByRole('link', { name: 'Play' })).toHaveAttribute('href', '/')
    expect(inPage().getByRole('link', { name: 'Create a game' })).toHaveAttribute('href', '/new')
  })

  describe('a code no open game has', () => {
    const gone = refused(404, 'not_found')

    it('ends in one sentence, and Play leads back to the open games', async () => {
      const app = await visit('/join/k7m2-9qxa', { join: gone })
      await guestJoins(app)
      expect(await screen.findByRole('alert')).toHaveTextContent('No open game with that code.')
      const play = inPage().getByRole('link', { name: 'Play' })
      await waitFor(() => expect(play).toHaveFocus())
      await app.user.click(play)
      expect(await screen.findByRole('heading', { level: 1, name: 'Play' })).toBeInTheDocument()
      expect(here(app)).toBe('/')
    })

    it('has a way to Create a game too', async () => {
      const app = await visit('/join/k7m2-9qxa', { join: gone })
      await guestJoins(app)
      await screen.findByRole('alert')
      await app.user.click(inPage().getByRole('link', { name: 'Create a game' }))
      expect(await screen.findByRole('form', { name: 'Create a game' })).toBeInTheDocument()
      expect(here(app)).toBe('/new')
    })
  })

  it('shows the wait the server names in Retry-After and keeps the form', async () => {
    const app = await visit('/join/k7m2-9qxa', {
      join: refused(429, 'too_many_attempts', { 'retry-after': '42' }),
    })
    await guestJoins(app)
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Too many wrong codes from this address. Try again in 42 seconds.',
    )
    expect(joinButton()).not.toHaveAttribute('aria-disabled')
    expect(screen.getByLabelText('Your name')).toHaveValue('Ivy')
  })

  it('says the server cannot be reached beside the shell’s offline notice, and joins on the second try', async () => {
    vi.spyOn(window.navigator, 'onLine', 'get').mockReturnValue(false)
    let reached = false
    const app = await visit('/join/k7m2-9qxa', {
      join: () =>
        reached
          ? jsonResponse(200, { game: MADE_GAME, seat: 1, token: tokenOf(1) })
          : Promise.reject(new TypeError('Failed to fetch')),
    })
    await act(async () => {
      window.dispatchEvent(new Event('offline'))
    })
    expect(screen.getByText('You are offline')).toBeInTheDocument()
    await guestJoins(app)
    expect(await screen.findByText(/Could not reach the server/)).toBeInTheDocument()
    expect(screen.getByLabelText('Your name')).toHaveValue('Ivy')
    reached = true
    await app.user.click(joinButton())
    await waitFor(() => expect(here(app)).toBe(`/game/${MADE_GAME}#seat=1&token=${tokenOf(1)}`))
  })
})
