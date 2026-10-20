// Goal: /dev/lobby reads seats, ready, away, me and names from the address (shrugging off nonsense), serves the real lobby over a fixture roster whose Ready and Not ready work, and answers "not found" with no fixture in a production build; the fixture's long names are as long as the server allows.
import { createMemoryHistory, createRouter, RouterProvider } from '@tanstack/react-router'
import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { clearFlash } from '@/lib/flash'
import { fixtureRoster, LOBBY_FIXTURE_MARKER } from './lobbyGame'
import { parseDevLobbySearch } from './lobbySearch'

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

const seatRows = (): HTMLElement[] =>
  within(screen.getByRole('list', { name: 'Seats' })).getAllByRole('listitem')
const states = (): (string | null)[] => seatRows().map((row) => row.getAttribute('data-state'))

afterEach(() => {
  cleanup()
  vi.unstubAllEnvs()
  vi.unstubAllGlobals()
  clearFlash()
})

describe('parseDevLobbySearch', () => {
  it.each([
    [{}, { seats: 4, ready: [], away: [], me: 0, names: undefined }],
    [{ seats: 6 }, { seats: 6, ready: [], away: [], me: 0, names: undefined }],
    [
      { seats: 2, ready: 1 },
      { seats: 2, ready: [1], away: [], me: 0, names: undefined },
    ],
    [
      { seats: 5, ready: '0,2', away: '4' },
      { seats: 5, ready: [0, 2], away: [4], me: 0, names: undefined },
    ],
    [
      { seats: 5, ready: [3, 1], away: 2, me: 3 },
      { seats: 5, ready: [1, 3], away: [2], me: 3, names: undefined },
    ],
    [
      { seats: 3, ready: '2,2,1', names: 'long' },
      { seats: 3, ready: [1, 2], away: [], me: 0, names: 'long' },
    ],
    [
      { seats: 2, visibility: 'invite' },
      { seats: 2, ready: [], away: [], me: 0, names: undefined, visibility: 'invite' },
    ],
    [
      { seats: 2, visibility: 'secret' },
      { seats: 2, ready: [], away: [], me: 0, names: undefined, visibility: undefined },
    ],
  ])('reads %j', (search, expected) => {
    expect(parseDevLobbySearch(search)).toEqual(expected)
  })

  it.each([
    [{ seats: 1 }, 4],
    [{ seats: 13 }, 4],
    [{ seats: 99 }, 4],
    [{ seats: 'many' }, 4],
    [{ seats: 2.5 }, 4],
    [{ seats: -3 }, 4],
    [{ seats: null }, 4],
  ])('takes %j for a table of four', (search, seats) => {
    expect(parseDevLobbySearch(search).seats).toBe(seats)
  })

  it('drops seats that are not at the table, and what is not a seat number', () => {
    const parsed = parseDevLobbySearch({ seats: 3, ready: '0,7,x,,-1,1.5,2', away: [5, 'a', 1], me: 9 })
    expect(parsed.ready).toEqual([0, 2])
    expect(parsed.away).toEqual([1])
    expect(parsed.me).toBe(0)
    expect(parseDevLobbySearch({ names: 'huge' }).names).toBeUndefined()
  })
})

describe('the fixture roster', () => {
  it('has the seats, flags and needed count the address asks for', () => {
    const roster = fixtureRoster({ seats: 4, ready: [0, 2], away: [3], me: 1 })
    expect(roster.seats.map((seat) => [seat.connected, seat.ready])).toEqual([
      [true, true],
      [true, false],
      [true, true],
      [false, false],
    ])
    expect(roster.needed).toBe(2)
  })

  it('has names of the longest length the server accepts for every seat, at any table size', () => {
    const roster = fixtureRoster({ seats: 12, ready: [], away: [], me: 0, names: 'long' })
    for (const { name } of roster.seats) {
      expect([...name].length, name).toBeGreaterThanOrEqual(31)
      expect([...name].length, name).toBeLessThanOrEqual(32)
    }
    expect(roster.seats[0]?.name).not.toContain(' ')
  })
})

describe('/dev/lobby in development', () => {
  it('shows a lobby of four seats when the address says nothing', async () => {
    await visit('/dev/lobby')
    await screen.findByRole('heading', { name: 'Waiting for players' })
    expect(seatRows()).toHaveLength(4)
    expect(states()).toEqual(['waiting', 'waiting', 'waiting', 'waiting'])
    expect(document.querySelector(`[data-fixture="${LOBBY_FIXTURE_MARKER}"]`)).not.toBeNull()
  }, 10_000)

  it.each([
    ['?seats=2&ready=1', ['waiting', 'ready']],
    ['?seats=3&ready=0,1&away=2', ['ready', 'ready', 'away']],
    ['?seats=6&ready=0&away=3,4,5', ['ready', 'waiting', 'waiting', 'away', 'away', 'away']],
    ['?seats=5&ready=4&away=4&me=2', ['waiting', 'waiting', 'waiting', 'waiting', 'ready']],
  ])('reads %s', async (search, expected) => {
    await visit(`/dev/lobby${search}`)
    await screen.findByRole('heading', { name: 'Waiting for players' })
    expect(states()).toEqual(expected)
  })

  it('seats this page at `me`, whatever the other seats are doing', async () => {
    await visit('/dev/lobby?seats=4&me=2&away=2')
    await screen.findByRole('heading', { name: 'Waiting for players' })
    const mine = seatRows().filter((row) => within(row).queryByText('you') !== null)
    expect(mine.map((row) => row.getAttribute('data-seat-row'))).toEqual(['2'])
    // This page's own seat is here, because the page is open, whatever the address says.
    expect(mine[0]?.getAttribute('data-state')).toBe('waiting')
  })

  it('lets Ready and Not ready be tried: the roster follows at once', async () => {
    await visit('/dev/lobby?seats=3&ready=1')
    await screen.findByRole('heading', { name: 'Waiting for players' })
    const user = userEvent.setup()
    expect(screen.getByText('1 of 3 ready, waiting for you and Cy to press Ready')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Ready' }))
    expect(states()).toEqual(['ready', 'ready', 'waiting'])
    expect(screen.getByText('2 of 3 ready, waiting for Cy to press Ready')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Not ready' }))
    expect(states()).toEqual(['waiting', 'ready', 'waiting'])
  })

  it('is the real lobby: the live region for announcements is there, and it says what the dev buttons change', async () => {
    await visit('/dev/lobby?seats=2')
    await screen.findByRole('heading', { name: 'Waiting for players' })
    const user = userEvent.setup()
    await user.click(screen.getByRole('button', { name: 'Ready' }))
    const region = document.querySelector('[data-slot="lobby-announcement"]')
    expect(region).toHaveAttribute('aria-live', 'polite')
    expect(region).toHaveTextContent('You are ready. 1 of 2 ready, waiting for Bo to press Ready')
  })

  it('gives seat 0 the host’s controls and every other seat a Leave game', async () => {
    await visit('/dev/lobby?seats=3')
    await screen.findByRole('heading', { name: 'Waiting for players' })
    expect(screen.getByRole('button', { name: 'Close game' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Remove Bo from the game' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Leave game' })).toBeNull()
    cleanup()
    await visit('/dev/lobby?seats=3&me=1')
    await screen.findByRole('heading', { name: 'Waiting for players' })
    expect(screen.getByRole('button', { name: 'Leave game' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Close game' })).toBeNull()
  })

  it('lets the host try the toggle and Remove: the fixture answers like a server', async () => {
    await visit('/dev/lobby?seats=3&visibility=invite')
    await screen.findByRole('heading', { name: 'Waiting for players' })
    const user = userEvent.setup()
    expect(screen.getByRole('button', { name: 'Invite only' })).toHaveAttribute('aria-pressed', 'true')
    await user.click(screen.getByRole('button', { name: 'Listed in the game browser' }))
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Listed in the game browser' })).toHaveAttribute(
        'aria-pressed',
        'true',
      ),
    )
    await user.click(screen.getByRole('button', { name: 'Remove Bo from the game' }))
    const question = await screen.findByRole('alertdialog', { name: 'Remove Bo?' })
    await user.click(within(question).getByRole('button', { name: 'Remove Bo' }))
    await waitFor(() => expect(seatRows()[1]).toHaveTextContent('not joined yet'))
    expect(screen.queryByRole('button', { name: 'Remove Bo from the game' })).toBeNull()
  })

  it('takes Close game to Play, where the notice says what happened', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response(JSON.stringify({ games: [] }), { status: 200 })),
    )
    const router = await visit('/dev/lobby?seats=2')
    await screen.findByRole('heading', { name: 'Waiting for players' })
    const user = userEvent.setup()
    await user.click(screen.getByRole('button', { name: 'Close game' }))
    const question = await screen.findByRole('alertdialog', { name: 'Close this game?' })
    await user.click(within(question).getByRole('button', { name: 'Close game' }))
    await waitFor(() => expect(router.state.location.pathname).toBe('/'))
    expect(await screen.findByRole('heading', { name: 'Play' })).toBeInTheDocument()
    expect(screen.getByRole('alert')).toHaveTextContent('You closed the game.')
  })

  it.each(['?seats=99', '?seats=1', '?seats=abc', '?ready=zz&away=!', '?me=40&seats=3'])(
    'ignores the nonsense in %s',
    async (search) => {
      await visit(`/dev/lobby${search}`)
      await screen.findByRole('heading', { name: 'Waiting for players' })
      expect(seatRows().length).toBeGreaterThanOrEqual(2)
      expect(seatRows().length).toBeLessThanOrEqual(12)
    },
  )
})

describe('/dev/lobby in a production build', () => {
  it('answers not found and draws no fixture', async () => {
    vi.stubEnv('DEV', false)
    await visit('/dev/lobby?seats=4')
    expect(await screen.findByText('Not Found')).toBeInTheDocument()
    expect(document.querySelector('[data-seat-row]')).toBeNull()
    expect(document.querySelector('[data-fixture]')).toBeNull()
  })
})
