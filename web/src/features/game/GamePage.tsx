import { useEffect, useMemo, useState } from 'react'
import type { ReactElement } from 'react'
import { useStore } from 'zustand'
import { useShallow } from 'zustand/react/shallow'
import { PageShell } from '@/components/PageShell'
import { usePageTitle } from '@/hooks/usePageTitle'
import { loadInvites } from '@/lib/invites'
import type { SeatLink } from '@/lib/seatLink'
import { api as sharedApi } from '@/net/api'
import type { Api } from '@/net/api'
import { decksApi as sharedDecks } from '@/net/decks'
import { preconsApi as sharedPrecons } from '@/net/precons'
import type { PreconsApi } from '@/net/precons'
import type { DecksApi } from '@/net/decks'
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
import { ReviewBar } from './ReviewBar'
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
  /**
   * The username of the signed-in account, used only when `link` is null: the page then joins through the session
   * cookie (`/ws/{game}`), which the server maps to the account's own seat. Null for a guest.
   */
  account?: string | null
  api?: Api
  /** The signed-in account's deck calls, for the deck chooser of the lobby: the shared client unless a test brings its own. */
  decks?: DecksApi
  /** The precons for the deck chooser: the shared client unless a test brings its own. */
  precons?: Pick<PreconsApi, 'list' | 'get'>
  /** Fakes for the socket, timers and origin; leave out in the app. */
  deps?: SessionOverrides
}

/** `/game/$gameId`: connects this seat and shows whichever screen the connection and the game call for. */
export function GamePage({
  gameId,
  link,
  account = null,
  api = sharedApi,
  decks = sharedDecks,
  precons = sharedPrecons,
  deps,
}: GamePageProps) {
  // A new game, seat, token or account is a new session, so the key makes it a new component rather than a stale one.
  if (link !== null) {
    return (
      <SeatedGame
        key={`${gameId}/${link.seat}/${link.token}`}
        gameId={gameId}
        token={link.token}
        name={joinName(gameId, link.seat)}
        api={api}
        decks={decks}
        precons={precons}
        deps={deps}
      />
    )
  }
  if (account === null) return <SeatLinkMissing gameId={gameId} />
  return (
    <SeatedGame
      key={`${gameId}/account/${account}`}
      gameId={gameId}
      token={undefined}
      name={account}
      api={api}
      decks={decks}
      precons={precons}
      deps={deps}
    />
  )
}

interface SeatedGameProps {
  gameId: string
  /** The seat token, or undefined to join through the session cookie. */
  token: string | undefined
  /** The name sent in `Join`. */
  name: string
  api: Api
  decks: DecksApi
  precons: Pick<PreconsApi, 'list' | 'get'>
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

function SeatedGame({ gameId, token, name, api, decks, precons, deps }: SeatedGameProps) {
  const [session] = useState(() => createSession({ ...deps, game: gameId, token, name }))
  useConnected(session)
  const state = useStore(session.store, useShallow(screenState))
  const lookup = useGameInfo(api, gameId, {
    poll: state.phase === 'lobby',
    timers: deps?.timers ?? browserTimers,
  })
  const context = useMemo(() => ({ game: gameId, session, lookup }), [gameId, session, lookup])

  const connection = connectionOf(state)
  const [review, setReview] = useState(false)
  const over = state.phase === 'over' && state.result !== null
  const ended =
    connection.kind === 'replaced' || connection.kind === 'stopped' || connection.kind === 'closed'

  // The screen is the game itself: the result, or what the seat sees once welcomed. Without one (still connecting,
  // or the seat was lost), the connection notice is the whole page.
  let screen: ReactElement | null = null
  let title = connection.kind === 'closed' ? 'Game closed' : ended ? 'Disconnected' : 'Connecting'
  if (over) {
    // The result closes for a look at the final table and its log, and comes back from the strip above them.
    screen = review ? <GameTable /> : <GameOver onReview={() => setReview(true)} />
    title = 'Game over'
  } else if (state.seat !== null && !ended) {
    screen =
      state.phase === 'playing' ? (
        <GameTable />
      ) : (
        <Lobby api={api} decks={decks} precons={precons} token={token} />
      )
    title = state.phase === 'playing' ? 'Game' : 'Lobby'
  }
  usePageTitle(useTurnTitle(session.store, title, screen !== null && state.phase === 'playing'))

  const retry = (): void => {
    session.disconnect()
    session.connect()
  }
  const connectionNotice =
    connection.kind !== 'open' && !over ? (
      <ConnectionNotice
        connection={connection}
        standalone={screen === null}
        missing={lookup.missing}
        via={token === undefined ? 'session' : 'link'}
        onRetry={retry}
      />
    ) : null
  const serverNotice =
    state.notice !== null && screen !== null && !over ? (
      <ServerNotice message={state.notice} onDismiss={session.dismissNotice} />
    ) : null
  // The table is exactly the window, so a notice over it floats under its top bar instead of pushing it down.
  const looking = over && review
  const floating = screen !== null && (state.phase === 'playing' || looking)
  return (
    <GameContext value={context}>
      {floating ? (
        <div
          data-slot="table-notices"
          className="pointer-events-none fixed inset-x-0 top-11 z-50 mx-auto grid max-w-page gap-2 px-4 empty:hidden"
        >
          {connectionNotice === null ? null : <div className="pointer-events-auto">{connectionNotice}</div>}
          {serverNotice === null ? null : <div className="pointer-events-auto">{serverNotice}</div>}
          {looking ? (
            <div className="pointer-events-auto">
              <ReviewBar onResult={() => setReview(false)} />
            </div>
          ) : null}
        </div>
      ) : (
        <>
          {connectionNotice === null ? null : (
            <PageShell className="pb-0 sm:pb-0">{connectionNotice}</PageShell>
          )}
          {serverNotice === null ? null : <PageShell className="pb-0 sm:pb-0">{serverNotice}</PageShell>}
        </>
      )}
      {screen}
    </GameContext>
  )
}
