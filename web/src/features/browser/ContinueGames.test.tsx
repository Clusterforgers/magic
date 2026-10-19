// Goal: a signed-in user sees a Continue link into each game of theirs that is running, named after the other players and opening the bare game page (their session opens the seat); a guest, a user with no running game and a failing listing see nothing, and a guest causes no request.
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
    initial: { status, user: status === 'signed-in' ? { id: 'u1', username: 'Ada' } : null },
  })
  await renderInRouter(
    <AuthContext.Provider value={store}>
      <PlayPage api={fakeLobbies([]).api} />
    </AuthContext.Provider>,
  )
  await screen.findByRole('heading', { name: 'No open games' })
}

const links = () => screen.queryAllByRole('link', { name: /^Continue/ })

describe('Continue', () => {
  it('links each running game, named after the others, to the game page', async () => {
    const games = [
      listed({ game: 'g1', status: 'running', others: [{ seat: 1, name: 'Bo', user: null }] }),
      listed({ game: 'g2', status: 'lobby' }),
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
    await waitFor(() => expect(links()).toHaveLength(2))
    expect(links().map((link) => [link.textContent, link.getAttribute('href')])).toEqual([
      ['Continue your game with Bo', '/game/g1'],
      ['Continue your game with Cy and diana', '/game/g4'],
    ])
  })

  it('shows nothing for a user whose games are not running', async () => {
    const myGames = vi.fn(() =>
      Promise.resolve([listed({ status: 'lobby' }), listed({ status: 'finished' })]),
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
