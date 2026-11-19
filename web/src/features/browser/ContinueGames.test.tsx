// Goal: a signed-in user sees a link into each game of theirs that is running (Continue) or in its lobby however full (Back to your lobby), named after the other players and opening the bare game page (their session opens the seat); a guest, a user with only finished games and a failing listing see nothing, a guest causes no request, and a lobby that closed since the last listing (an ended Delves run) is never linked while the list reloads or after a failed reload.
import { screen, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { AuthContext } from '@/features/auth/context'
import { createAuthStore, offlineAccounts } from '@/features/auth/store'
import type { AuthStatus } from '@/features/auth/store'
import type { MyGame } from '@/protocol/generated'
import { listed } from '@/test-utils/accountsServer'
import { fakeLobbies } from '@/test-utils/lobbies'
import { renderInRouter } from '@/test-utils/router'
import { PlayPage } from './PlayPage'

async function open(status: AuthStatus, myGames: () => Promise<MyGame[]>) {
  const store = createAuthStore({
    api: { ...offlineAccounts, myGames },
    initial: { status, user: status === 'signed-in' ? { id: 'u1', username: 'Ada', is_admin: false } : null },
  })
  await renderInRouter(
    <AuthContext.Provider value={store}>
      <PlayPage api={fakeLobbies([]).api} />
    </AuthContext.Provider>,
  )
  await screen.findByRole('heading', { name: 'No open games' })
}

const links = () => screen.queryAllByRole('link', { name: /^(Continue|Back to your lobby)/ })

describe('Continue', () => {
  it('links each running game and each lobby, named after the others, to the game page', async () => {
    const games = [
      listed({ game: 'g1', status: 'running', others: [{ seat: 1, name: 'Bo', user: null }] }),
      listed({ game: 'g2', status: 'lobby', others: [{ seat: 1, name: 'Bot', user: null }] }),
      listed({ game: 'g3', status: 'finished' }),
      listed({
        game: 'g4',
        status: 'running',
        others: [
          { seat: 1, name: 'Cy', user: null },
          { seat: 2, name: 'Di', user: 'diana' },
        ],
      }),
    ]
    await open('signed-in', () => Promise.resolve(games))
    await waitFor(() => expect(links()).toHaveLength(3))
    expect(links().map((link) => [link.textContent, link.getAttribute('href')])).toEqual([
      ['Continue your game with Bo', '/game/g1'],
      ['Back to your lobby with Bot', '/game/g2'],
      ['Continue your game with Cy and diana', '/game/g4'],
    ])
  })

  it('shows nothing for a user whose games are over', async () => {
    const myGames = vi.fn(() =>
      Promise.resolve([listed({ status: 'finished' }), listed({ status: 'failed' })]),
    )
    await open('signed-in', myGames)
    await waitFor(() => expect(myGames).toHaveBeenCalledTimes(1))
    expect(links()).toHaveLength(0)
  })

  it('shows nothing, and no error, when the listing fails', async () => {
    const myGames = vi.fn(() => Promise.reject(new Error('down')))
    await open('signed-in', myGames)
    await waitFor(() => expect(myGames).toHaveBeenCalledTimes(1))
    expect(links()).toHaveLength(0)
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })

  it('drops a lobby that closed since the last listing, never showing the stale link meanwhile', async () => {
    // The first listing has a Delves fight lobby; the run ends and the next listing no longer has it.
    const fight = listed({
      game: 'g1',
      status: 'lobby',
      others: [{ seat: 1, name: 'Seraphine', user: null }],
    })
    let answer: (games: MyGame[]) => void = () => {}
    const myGames = vi
      .fn<() => Promise<MyGame[]>>()
      .mockResolvedValueOnce([fight])
      .mockImplementationOnce(() => new Promise((resolve) => (answer = resolve)))
    const store = createAuthStore({
      api: { ...offlineAccounts, myGames },
      initial: { status: 'signed-in', user: { id: 'u1', username: 'Ada', is_admin: false } },
    })
    await store.getState().loadGames()
    expect(store.getState().games.games).toHaveLength(1)
    await renderInRouter(
      <AuthContext.Provider value={store}>
        <PlayPage api={fakeLobbies([]).api} />
      </AuthContext.Provider>,
    )
    await screen.findByRole('heading', { name: 'No open games' })
    await waitFor(() => expect(myGames).toHaveBeenCalledTimes(2))
    expect(links()).toHaveLength(0)
    answer([])
    await waitFor(() => expect(store.getState().games.status).toBe('ready'))
    expect(links()).toHaveLength(0)
  })

  it('shows no stale link when a reload fails', async () => {
    const myGames = vi
      .fn<() => Promise<MyGame[]>>()
      .mockResolvedValueOnce([listed({ game: 'g1', status: 'lobby' })])
      .mockRejectedValueOnce(new Error('down'))
    const store = createAuthStore({
      api: { ...offlineAccounts, myGames },
      initial: { status: 'signed-in', user: { id: 'u1', username: 'Ada', is_admin: false } },
    })
    await store.getState().loadGames()
    await renderInRouter(
      <AuthContext.Provider value={store}>
        <PlayPage api={fakeLobbies([]).api} />
      </AuthContext.Provider>,
    )
    await waitFor(() => expect(store.getState().games.status).toBe('failed'))
    expect(links()).toHaveLength(0)
  })

  it.each(['signed-out', 'unavailable'] as const)(
    'asks nothing and shows nothing for a guest (%s)',
    async (status) => {
      const myGames = vi.fn(() => Promise.resolve([listed()]))
      await open(status, myGames)
      expect(myGames).not.toHaveBeenCalled()
      expect(links()).toHaveLength(0)
    },
  )
})
