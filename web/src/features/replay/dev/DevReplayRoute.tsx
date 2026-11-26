import { useSearch } from '@tanstack/react-router'
import { useMemo } from 'react'
import type { Fetch } from '@/net/api'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { ReplayPage } from '../ReplayPage'
import { commanderReplay, duelReplay, REPLAY_FIXTURE_MARKER } from './fixture'
import type { DevGame, DevView } from './replaySearch'

/** The id the dev replay is loaded under. */
const DEV_REPLAY_GAME = '00000000-0000-4000-8000-00000000dev1'

/** The `fetch` the replay page is given for a view: it answers the replay request without a server. */
function devReplayFetch(view: DevView, game: DevGame): Fetch {
  return async () => {
    switch (view) {
      case 'loading':
        return new Promise<never>(() => undefined)
      case 'missing':
        return jsonResponse(404, { error: 'not_found', message: 'no replay' })
      case 'signed-out':
        return jsonResponse(401, { error: 'unauthenticated', message: 'Sign in first.' })
      case 'offline':
        throw new TypeError('Failed to fetch')
      case 'broken':
        return jsonResponse(200, { steps: 'none' })
      case 'game':
        return jsonResponse(200, game === 'commander' ? commanderReplay() : duelReplay())
    }
  }
}

/**
 * `/dev/replay?game=duel`: the real replay page over a replay in memory. `game` is `duel` (a recorded two-seat game)
 * or `commander` (the dev table's four-seat board); `step` opens it at a step (`last` for the end); `view` is `game`,
 * `loading`, `missing`, `signed-out`, `offline` or `broken`.
 */
export function DevReplayRoute() {
  const { view, game, step } = useSearch({ from: '/dev/replay' })
  const fetch = useMemo(() => devReplayFetch(view, game), [view, game])
  return (
    <>
      <p className="sr-only" data-fixture={REPLAY_FIXTURE_MARKER}>
        Development replay
      </p>
      <ReplayPage
        key={`${view}-${game}-${step}`}
        game={DEV_REPLAY_GAME}
        fetch={fetch}
        start={step === 'last' ? Number.MAX_SAFE_INTEGER : step}
      />
    </>
  )
}
