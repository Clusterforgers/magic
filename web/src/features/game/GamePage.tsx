import { useEffect, useMemo, useState } from 'react'
import type { ReactElement } from 'react'
import { useStore } from 'zustand'
import { useShallow } from 'zustand/react/shallow'
import { usePageTitle } from '@/hooks/usePageTitle'
import { loadInvites } from '@/lib/invites'
import type { SeatLink } from '@/lib/seatLink'
import { api as sharedApi } from '@/net/api'
import type { Api } from '@/net/api'
import { browserTimers } from '@/net/timers'
import { createSession } from '@/play/session'
import type { Session, SessionDeps } from '@/play/session'
import type { PlayState } from '@/play/store'
import { GameTable } from '@/features/table/GameTable'
import { connectionOf } from './connection'
import { ConnectionNotice } from './ConnectionNotice'
import { GameContext } from './gameContext'
import { GameOver } from './GameOver'
import { Lobby } from './Lobby'
import { SeatLinkMissing } from './SeatLinkMissing'
import { ServerNotice } from './ServerNotice'
import { useGameInfo } from './useGameInfo'
import { useTurnTitle } from './useTurnTitle'

/** The parts of `SessionDeps` a caller may replace, such as the socket and the clock in tests. */
export type SessionOverrides = Omit<SessionDeps, 'game' | 'token' | 'name'>

/** Props of `GamePage`. */
export interface GamePageProps {
  /** The game's id, from the route. */
  gameId: string
  /** The seat and token from the address fragment, or null when there was none or it could not be read. */
  link: SeatLink | null
  api?: Api
  /** Fakes for the socket, timers and origin; leave out in the app. */
  deps?: SessionOverrides
}

/** `/game/$gameId`: connects this seat and shows whichever screen the connection and the game call for. */
export function GamePage({ gameId, link, api = sharedApi, deps }: GamePageProps) {
  if (link === null) return <SeatLinkMissing />
  // A new game, seat or token is a new session, so the key makes it a new component rather than a stale one.
  return (
    <SeatedGame
      key={`${gameId}/${link.seat}/${link.token}`}
      gameId={gameId}
      link={link}
      api={api}
      deps={deps}
    />
  )
}

interface SeatedGameProps {
  gameId: string
  link: SeatLink
  api: Api
  deps: SessionOverrides | undefined
}

// Only what decides the screen, so the many store updates of a running game do not re-render this component.
const screenState = (state: PlayState) => ({
  status: state.status,
  attempt: state.attempt,
  seat: state.seat,
  phase: state.phase,
  result: state.result,
  notice: state.notice,
})

/** The name sent in `Join`: the one the host gave the seat when this tab made the game, else "Seat N". */
function joinName(gameId: string, seat: number): string {
  return loadInvites(gameId)?.seats.find((invite) => invite.seat === seat)?.name ?? `Seat ${seat + 1}`
}

/** Keeps `session` connected while the page is mounted. */
function useConnected(session: Session): void {
  useEffect(() => {
    let mounted = true
    // One microtask later, so StrictMode's mount, unmount, mount probe opens one socket and not a throwaway first.
    queueMicrotask(() => {
      if (mounted) session.connect()
    })
    return () => {
      mounted = false
      session.disconnect()
    }
  }, [session])
}

function SeatedGame({ gameId, link, api, deps }: SeatedGameProps) {
  const [session] = useState(() =>
    createSession({ ...deps, game: gameId, token: link.token, name: joinName(gameId, link.seat) }),
  )
  useConnected(session)
  const state = useStore(session.store, useShallow(screenState))
  const lookup = useGameInfo(api, gameId, {
    poll: state.phase === 'lobby',
    timers: deps?.timers ?? browserTimers,
  })
  const context = useMemo(() => ({ game: gameId, session, lookup }), [gameId, session, lookup])

  const connection = connectionOf(state)
  const over = state.phase === 'over' && state.result !== null
  const ended = connection.kind === 'replaced' || connection.kind === 'stopped'

  // The screen is the game itself: the result, or what the seat sees once welcomed. Without one (still connecting,
  // or the seat was lost), the connection notice is the whole page.
  let screen: ReactElement | null = null
  let title = ended ? 'Disconnected' : 'Connecting'
  if (over) {
    screen = <GameOver />
    title = 'Game over'
  } else if (state.seat !== null && !ended) {
    screen = state.phase === 'playing' ? <GameTable /> : <Lobby />
    title = state.phase === 'playing' ? 'Game' : 'Lobby'
  }
  usePageTitle(useTurnTitle(session.store, title, screen !== null && state.phase === 'playing'))

  const retry = (): void => {
    session.disconnect()
    session.connect()
  }
  return (
    <GameContext value={context}>
      {connection.kind !== 'open' && !over ? (
        <div className="mx-auto w-full max-w-2xl px-4 pt-6 sm:px-6">
          <ConnectionNotice
            connection={connection}
            standalone={screen === null}
            missing={lookup.missing}
            onRetry={retry}
          />
        </div>
      ) : null}
      {state.notice !== null && screen !== null && !over ? (
        <div className="mx-auto w-full max-w-2xl px-4 pt-6 sm:px-6">
          <ServerNotice message={state.notice} onDismiss={session.dismissNotice} />
        </div>
      ) : null}
      {screen}
    </GameContext>
  )
}
