import { useSearch } from '@tanstack/react-router'
import { useMemo } from 'react'
import { GameContext } from '@/features/game/gameContext'
import { Lobby } from '@/features/game/Lobby'
import { lobbyApi, lobbyGame, LOBBY_FIXTURE_MARKER } from './lobbyGame'

/**
 * `/dev/lobby?seats=4&ready=0,2&away=3&me=1&names=long&visibility=invite`: the real lobby over a fixture roster, for
 * looking at it. `seats` is 2 to 12; `ready` and `away` list seat numbers; `me` is the seat this page plays (0 is the
 * host, who gets the toggle, Remove and Close; any other seat gets Leave); `names=long` gives every seat a name of 32
 * characters; `visibility=invite` makes the game invite only. Ready, the toggle and Remove work on the fixture as if a
 * server answered them; Close and Leave go to Play.
 */
export function DevLobbyRoute() {
  const { seats, ready, away, me, names, visibility } = useSearch({ from: '/dev/lobby' })
  const game = useMemo(
    () => lobbyGame({ seats, ready, away, me, names, visibility }),
    [seats, ready, away, me, names, visibility],
  )
  const api = useMemo(() => lobbyApi(game.session), [game])
  return (
    <GameContext value={game}>
      <p className="sr-only" data-fixture={LOBBY_FIXTURE_MARKER}>
        Development lobby with {seats} seats
      </p>
      <Lobby api={api} token={undefined} />
    </GameContext>
  )
}
