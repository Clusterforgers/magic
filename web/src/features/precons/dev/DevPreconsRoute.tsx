import { useSearch } from '@tanstack/react-router'
import { useMemo } from 'react'
import { AuthContext } from '@/features/auth/context'
import { createAuthStore, offlineAccounts } from '@/features/auth/store'
import { ApiError } from '@/net/api'
import type { Api } from '@/net/api'
import { cardDetail } from '@/features/builder/fakeCards'
import type { CardsApi } from '@/net/cards'
import { PreconError } from '@/net/precons'
import type { PreconsApi } from '@/net/precons'
import { PreconsPage } from '../PreconsPage'
import { FIXTURE_PRECONS, fixtureView, PRECONS_FIXTURE_MARKER } from './fixture'
import type { DevView } from './preconsSearch'

const never = <T,>(signal?: AbortSignal): Promise<T> =>
  new Promise((_, reject) => signal?.addEventListener('abort', () => reject(new Error('aborted'))))

function devApi(view: DevView): PreconsApi {
  return {
    list: (signal) => {
      switch (view) {
        case 'empty':
          return Promise.resolve({ precons: [] })
        case 'loading':
          return never(signal)
        case 'error':
          return Promise.reject(new PreconError('network', 0, 'offline'))
        case 'unavailable':
          return Promise.reject(new PreconError('cards_unavailable', 503, 'no database'))
        case 'list':
          return Promise.resolve({ precons: [...FIXTURE_PRECONS] })
      }
    },
    get: (code) => {
      const found = fixtureView(code)
      return found === null
        ? Promise.reject(new PreconError('not_found', 404, 'no precon has that code'))
        : Promise.resolve(found)
    },
  }
}

// Every card reads the same made-up text, long enough for the fixture's spans to quote something.
const MADE_UP = Array.from(
  { length: 5 },
  (_, index) => `Clause ${index + 1} of the made-up text reads like this.`,
).join(' ')
const devCards: Pick<CardsApi, 'get'> = {
  get: (id) =>
    Promise.resolve(cardDetail(id, { faces: [{ ...cardDetail(id).faces[0]!, oracle_text: MADE_UP }] })),
}

const devGames = (refused: boolean): Pick<Api, 'createGame'> => ({
  createGame: () =>
    refused
      ? Promise.reject(new ApiError('unknown', 422, 'that deck cannot be played in this game'))
      : Promise.resolve({ game: 'dev-game', code: 'K7M29QXA', seats: [{ seat: 0, token: 'dev-token' }] }),
})

/**
 * `/dev/precons?view=list&account=1&play=refused`: the real precon page over a fixed list in memory, for looking at
 * it. `view` is `list` (every check state, long names, two commanders and none), `empty`, `loading` (the list never
 * arrives), `error` or `unavailable`; `account=1` makes this page a signed-in player, who plays in one click, where a
 * guest is asked for a name; `play=refused` ends every Play in the server's 422. A name opens the details of its
 * precon, whose cards are made up.
 */
export function DevPreconsRoute() {
  const { view, account, play } = useSearch({ from: '/dev/precons' })
  const api = useMemo(() => devApi(view), [view])
  const games = useMemo(() => devGames(play === 'refused'), [play])
  const auth = useMemo(
    () =>
      createAuthStore({
        api: { ...offlineAccounts, myGames: async () => [] },
        initial:
          account === true
            ? { status: 'signed-in', user: { id: 'dev-ada', username: 'Ada_L' } }
            : { status: 'unavailable' },
      }),
    [account],
  )
  return (
    <AuthContext value={auth}>
      <p className="sr-only" data-fixture={PRECONS_FIXTURE_MARKER}>
        Development precon list
      </p>
      <PreconsPage api={api} games={games} cards={devCards} />
    </AuthContext>
  )
}
