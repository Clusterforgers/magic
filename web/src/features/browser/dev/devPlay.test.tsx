// Goal: /dev/play reads its view and account from the address (shrugging off nonsense), serves the real Play page over the fixture's open games (one of every kind, or forty more with the filters), shows an empty list, a wait and a failed list, offers a signed-in player's games to continue, refuses a Join without leaving the page, and answers "not found" with no fixture in a production build.
import { createMemoryHistory, createRouter, RouterProvider } from '@tanstack/react-router'
import { act, cleanup, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { FIXTURE_LOBBIES, LONG_HOST, manyLobbies, PLAY_FIXTURE_MARKER } from './fixture'
import { DEV_VIEWS, parseDevPlaySearch } from './playSearch'

/** The real route tree, loaded fresh so the route sees the `import.meta.env.DEV` the test has set. */
async function visit(url: string) {
  vi.resetModules()
  const { routeTree } = await import('@/routeTree.gen')
  const router = createRouter({ routeTree, history: createMemoryHistory({ initialEntries: [url] }) })
  render(<RouterProvider router={router} />)
  await act(async () => {
    await router.load()
  })
  return router
}

afterEach(() => {
  cleanup()
  vi.unstubAllEnvs()
})

const games = () => screen.findByRole('list', { name: 'Open games, newest first' })

describe('parseDevPlaySearch', () => {
  it.each([
    [{}, { view: 'list' }],
    [
      { view: 'many', account: '1' },
      { view: 'many', account: true },
    ],
    [
      { view: 'empty', account: 1 },
      { view: 'empty', account: true },
    ],
    [{ view: 'nonsense', account: 'yes' }, { view: 'list' }],
    [{ view: 3 }, { view: 'list' }],
  ])('reads %j as %j', (search, parsed) => {
    expect(parseDevPlaySearch(search)).toEqual(parsed)
  })

  it('knows every view the route draws', () => {
    expect(DEV_VIEWS).toEqual(['list', 'many', 'empty', 'loading', 'error'])
  })
})

describe('the fixture', () => {
  it('holds a running, a full, a seated, a Commander and a by-hand game, each id once', () => {
    expect(FIXTURE_LOBBIES.some((game) => game.running === true)).toBe(true)
    expect(FIXTURE_LOBBIES.some((game) => game.seated === true)).toBe(true)
    expect(FIXTURE_LOBBIES.some((game) => game.manual_play)).toBe(true)
    expect(FIXTURE_LOBBIES.some((game) => (game.commanders?.length ?? 0) > 0)).toBe(true)
    expect(FIXTURE_LOBBIES.some((game) => game.running !== true && game.filled >= game.total)).toBe(true)
    const ids = manyLobbies(40).map((game) => game.game)
    expect(new Set(ids).size).toBe(ids.length)
  })
})

describe('/dev/play', () => {
  it('draws the real page with one card per open game', async () => {
    await visit('/dev/play')
    expect(await screen.findByRole('heading', { level: 1, name: 'Play' })).toBeInTheDocument()
    const list = await games()
    expect(within(list).getAllByRole('article')).toHaveLength(FIXTURE_LOBBIES.length)
    expect(within(list).getByRole('article', { name: LONG_HOST })).toBeInTheDocument()
    expect(document.querySelector(`[data-fixture="${PLAY_FIXTURE_MARKER}"]`)).not.toBeNull()
    expect(screen.queryByRole('region', { name: 'Your games' })).toBeNull()
  })

  it('lists forty more games with the filters in the many view', async () => {
    await visit('/dev/play?view=many')
    const list = await games()
    expect(within(list).getAllByRole('article')).toHaveLength(FIXTURE_LOBBIES.length + 40)
    expect(screen.getByRole('switch', { name: 'Hide full games' })).toBeInTheDocument()
  })

  it('offers a signed-in player the games to continue', async () => {
    await visit('/dev/play?account=1')
    expect(await screen.findByRole('link', { name: 'Continue your game with Bo' })).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Back to your lobby' })).toBeInTheDocument()
  })

  it('refuses a Join and stays on the page saying why', async () => {
    const router = await visit('/dev/play?account=1')
    const list = await games()
    await userEvent.click(within(list).getByRole('button', { name: "Join Bo's game, 1 of 2 seats" }))
    expect(await screen.findByText("Could not join Bo's game")).toBeInTheDocument()
    expect(router.state.location.pathname).toBe('/dev/play')
  })

  it.each([
    ['empty', 'No open games'],
    ['loading', 'Loading the open games…'],
    ['error', 'Could not load the open games'],
  ])('draws the %s view', async (view, words) => {
    await visit(`/dev/play?view=${view}`)
    expect(await screen.findByText(words)).toBeInTheDocument()
  })
})

describe('/dev/play in a production build', () => {
  it('answers not found and draws no fixture', async () => {
    vi.stubEnv('DEV', false)
    await visit('/dev/play')
    expect(await screen.findByText('Not Found')).toBeInTheDocument()
    expect(document.querySelector('[data-fixture]')).toBeNull()
  })
})
