import { useSearch } from '@tanstack/react-router'
import { useMemo } from 'react'
import { AuthContext } from '@/features/auth/context'
import { createAuthStore, offlineAccounts } from '@/features/auth/store'
import type { Api, LobbyListing } from '@/net/api'
import { LobbyError } from '@/net/lobbyErrors'
import { PlayPage } from '../PlayPage'
import { FIXTURE_LOBBIES, FIXTURE_MY_GAMES, manyLobbies, PLAY_FIXTURE_MARKER } from './fixture'
import type { DevView } from './playSearch'

const never = new Promise<never>(() => undefined)

/**
 * The game browser of a view, in memory. The listing keeps one `ETag`, so a poll draws nothing new. A Join is refused
 * as full, so the page stays where it is and shows how it says so.
 */
function devLobbies(view: DevView): Pick<Api, 'listLobbies' | 'joinGame' | 'joinByCode'> {
  const games = view === 'empty' ? [] : view === 'many' ? manyLobbies(40) : [...FIXTURE_LOBBIES]
  let listing: LobbyListing | null = null
  return {
    listLobbies: async (previous) => {
      if (view === 'loading') return never
      if (view === 'error') throw new LobbyError('network', 0, 'Failed to fetch')
      if (previous !== undefined) return previous
      listing ??= { games, etag: '"dev"', fetchedAt: Date.now() }
      return listing
    },
    joinGame: async () => {
      throw new LobbyError('full', 409, 'Every seat is taken.')
    },
    joinByCode: async () => {
      throw new LobbyError('not_found', 404, 'No game has that code.')
    },
  }
}

/**
 * `/dev/play?view=list`: the real Play page over open games in memory. `view` is `list` (one row of every kind),
 * `many` (forty more), `empty`, `loading` or `error`; `account=1` signs Ada in, with two games to continue.
 */
export function DevPlayRoute() {
  const { view, account } = useSearch({ from: '/dev/play' })
  const api = useMemo(() => devLobbies(view), [view])
  const auth = useMemo(
    () =>
      createAuthStore({
        api: { ...offlineAccounts, myGames: async () => [...FIXTURE_MY_GAMES] },
        initial:
          account === true
            ? { status: 'signed-in', user: { id: 'dev-ada', username: 'Ada_L', is_admin: false } }
            : { status: 'signed-out' },
      }),
    [account],
  )
  return (
    <AuthContext value={auth}>
      <p className="sr-only" data-fixture={PLAY_FIXTURE_MARKER}>
        Development list of open games
      </p>
      <PlayPage key={view} api={api} />
    </AuthContext>
  )
}
