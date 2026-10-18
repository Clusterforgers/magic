import { useSearch } from '@tanstack/react-router'
import { useMemo } from 'react'
import { GameContext } from '@/features/game/gameContext'
import { Lobby } from '@/features/game/Lobby'
import { lobbyGame, LOBBY_FIXTURE_MARKER } from './lobbyGame'

/**
 * `/dev/lobby?seats=4&ready=0,2&away=3&me=1&names=long`: the real lobby over a fixture roster, for looking at it.
 * `seats` is 2 to 12; `ready` and `away` list seat numbers; `me` is the seat this page plays; `names=long` gives every
 * seat a name of 32 characters. Ready and Not ready work on this page's seat, as if a server answered them.
 */
export function DevLobbyRoute() {
  const { seats, ready, away, me, names } = useSearch({ from: '/dev/lobby' })
  const game = useMemo(() => lobbyGame({ seats, ready, away, me, names }), [seats, ready, away, me, names])
  return (
    <GameContext value={game}>
      <p className="sr-only" data-fixture={LOBBY_FIXTURE_MARKER}>
        Development lobby with {seats} seats
      </p>
      <Lobby />
    </GameContext>
  )
}
