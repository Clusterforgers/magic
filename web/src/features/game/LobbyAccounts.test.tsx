// Goal: the lobby shows which account sits in which seat, and that this seat is saved to the signed-in account, as far as the user's own game list says; it shows nothing to a guest, asks the list again when somebody new connects, and never fails the lobby when the list cannot be had.
import { screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { authError, listed, startAccounts } from '@/test-utils/accountsServer'
import type { AccountsScript } from '@/test-utils/accountsServer'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { GAME_ID, renderGame } from '@/test-utils/gameHarness'
import { createAuthStore } from '@/features/auth/store'
import { LOBBY_POLL_MS } from './useGameInfo'

afterEach(() => {
  vi.unstubAllGlobals()
  window.sessionStorage.clear()
})

const ADA = { username: 'Ada_L', password: 'correct horse battery' }
const users = { [ADA.username]: ADA.password }

const info = (connected: number[]) => ({
  started: false,
  seats: [
    { seat: 0, name: 'Ana' },
    { seat: 1, name: 'Bo' },
  ],
  connected,
  result: null,
})
const rows = () => within(screen.getByRole('list', { name: 'Seats' })).getAllByRole('listitem')
const myGamesRequests = (accounts: ReturnType<typeof startAccounts>) =>
  accounts.calls.filter((call) => call.url === '/api/me/games')
const mine = (over: Parameters<typeof listed>[0] = {}) =>
  listed({
    game: GAME_ID,
    seat: 0,
    name: 'Ana',
    status: 'lobby',
    others: [{ seat: 1, name: 'Bo', user: 'bo' }],
    ...over,
  })

async function openLobby(accounts: AccountsScript = {}, connected: number[][] = [[1]]) {
  const server = startAccounts({ users, signedIn: ADA.username, ...accounts })
  const auth = createAuthStore({ api: server.api })
  let asked = 0
  const h = await renderGame({
    auth,
    script: { game: () => jsonResponse(200, info(connected[Math.min(asked++, connected.length - 1)] ?? [])) },
  })
  h.join(0, 2)
  await h.settle()
  return { h, server, auth }
}

describe('a signed-in player whose seat is saved to the account', () => {
  it('sees their own account on their seat and the other account on the other', async () => {
    await openLobby({ games: [mine()] })
    await waitFor(() => expect(rows()[1]).toHaveTextContent('signed in as bo'))
    expect(rows()[0]).toHaveTextContent('Seat 1Anayouhostsigned in as Ada_L')
    expect(rows()[1]).toHaveTextContent('Seat 2Bosigned in as bo')
  })

  it('says the seat is saved and points at My games', async () => {
    await openLobby({ games: [mine()] })
    const saved = await waitFor(() => {
      const note = document.querySelector('[data-slot="seat-saved"]')
      if (note === null) throw new Error('not yet')
      return note as HTMLElement
    })
    expect(saved).toHaveTextContent('You are signed in as Ada_L. This seat is saved to your account')
    expect(within(saved).getByRole('link', { name: 'My games' })).toHaveAttribute('href', '/games')
  })

  it('shows a seat whose occupant is a guest as it always was, with no account', async () => {
    await openLobby({ games: [mine({ others: [{ seat: 1, name: 'Bo', user: null }] })] })
    await waitFor(() => expect(rows()[0]).toHaveTextContent('signed in as Ada_L'))
    expect(rows()[1]).not.toHaveTextContent('signed in')
  })

  it('prints the username as plain text', async () => {
    await openLobby({ games: [mine({ others: [{ seat: 1, name: 'Bo', user: '<b>bo</b>' }] })] })
    await waitFor(() => expect(rows()[1]).toHaveTextContent('signed in as <b>bo</b>'))
    expect(rows()[1]?.querySelector('b')).toBeNull()
  })

  it('asks the list again when somebody new connects, and shows the account that took the seat', async () => {
    const { h, server } = await openLobby(
      { games: [mine({ others: [{ seat: 1, name: 'Bo', user: null }] })] },
      [[], [1]],
    )
    await waitFor(() => expect(rows()[0]).toHaveTextContent('signed in as Ada_L'))
    expect(rows()[1]).not.toHaveTextContent('signed in as')
    const before = myGamesRequests(server).length
    server.setGames([mine()]) // Bo opened a seat link while signed in: the server saved the seat to bo
    await h.advance(LOBBY_POLL_MS)
    await waitFor(() => expect(rows()[1]).toHaveTextContent('signed in as bo'))
    expect(myGamesRequests(server).length).toBeGreaterThan(before)
  })
})

describe('a signed-in player whose seat is not in their list', () => {
  it('shows no accounts and no note when the game is not listed', async () => {
    const { server } = await openLobby({ games: [] })
    await waitFor(() => expect(myGamesRequests(server).length).toBeGreaterThan(0))
    await waitFor(() => expect(server.calls.length).toBeGreaterThan(1))
    expect(rows()[0]).not.toHaveTextContent('signed in')
    expect(document.querySelector('[data-slot="seat-saved"]')).toBeNull()
  })

  it('does not call a seat saved when the account holds another seat of the game', async () => {
    const { server } = await openLobby({
      games: [mine({ seat: 1, name: 'Bo', others: [{ seat: 0, name: 'Ana', user: null }] })],
    })
    await waitFor(() => expect(myGamesRequests(server).length).toBeGreaterThan(0))
    await waitFor(() => expect(server.calls.length).toBeGreaterThan(1))
    expect(rows()[0]).not.toHaveTextContent('signed in as')
    expect(document.querySelector('[data-slot="seat-saved"]')).toBeNull()
  })

  it('keeps the lobby as it was, with no alert, when the list cannot be had', async () => {
    const { server } = await openLobby({ override: { games: () => authError(500, 'internal') } })
    await waitFor(() => expect(myGamesRequests(server).length).toBeGreaterThan(0))
    expect(screen.queryByRole('alert')).toBeNull()
    expect(screen.getByRole('heading', { name: 'Waiting for players' })).toBeInTheDocument()
    expect(rows()).toHaveLength(2)
    expect(document.querySelector('[data-slot="seat-account"]')).toBeNull()
  })
})

describe('a guest', () => {
  it('sees no accounts, no note, and the list is never asked for', async () => {
    const server = startAccounts({ users, games: [mine()] })
    const auth = createAuthStore({ api: server.api, initial: { status: 'signed-out' } })
    const h = await renderGame({ auth, script: { game: () => jsonResponse(200, info([1])) } })
    h.join(0, 2)
    await h.settle()
    await h.advance(LOBBY_POLL_MS * 2)
    expect(document.querySelector('[data-slot="seat-account"]')).toBeNull()
    expect(document.querySelector('[data-slot="seat-saved"]')).toBeNull()
    expect(myGamesRequests(server)).toHaveLength(0)
  })

  it('is just as before on a page without any sign-in state', async () => {
    const h = await renderGame({ script: { game: () => jsonResponse(200, info([1])) } })
    h.join(0, 2)
    await h.settle()
    expect(rows()[0]).toHaveTextContent('Seat 1Anayou')
    expect(document.querySelector('[data-slot="seat-account"]')).toBeNull()
  })
})
