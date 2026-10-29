import { useSearch } from '@tanstack/react-router'
import { useMemo } from 'react'
import { AuthContext } from '@/features/auth/context'
import { createAuthStore, offlineAccounts } from '@/features/auth/store'
import { GameContext } from '@/features/game/gameContext'
import { Lobby } from '@/features/game/Lobby'
import { deckFacts, lobbyDecks, lobbyPrecons } from './deckApis'
import { lobbyApi, lobbyGame, LOBBY_FIXTURE_MARKER } from './lobbyGame'

/**
 * `/dev/lobby?seats=4&ready=0,2&away=3&me=1&names=long&visibility=invite&nodeck=1&account=1`: the real lobby over a
 * fixture roster, for looking at it. `seats` is 2 to 12; `ready` and `away` list seat numbers; `me` is the seat this
 * page plays (0 is the host, who gets the toggle, Remove and Close; any other seat gets Leave); `names=long` gives
 * every seat a name of 32 characters; `visibility=invite` makes the game invite only; `nodeck` lists the seats that
 * have chosen no deck yet; `account=1` makes this page a signed-in player, whose chooser lists stored decks beside the
 * built-in ones; `format=commander` makes it a Commander game and `rules=1` gives the game house rules. Ready, the toggle, Remove and the choice of a deck work on the fixture as if a server answered them;
 * Close and Leave go to Play.
 */
export function DevLobbyRoute() {
  const {
    seats,
    ready,
    away,
    me,
    names,
    visibility,
    nodeck,
    account,
    format,
    rules,
    decks: many,
    list,
    detail,
  } = useSearch({
    from: '/dev/lobby',
  })
  const game = useMemo(
    () => lobbyGame({ seats, ready, away, me, names, visibility, nodeck, format, rules }),
    [seats, ready, away, me, names, visibility, nodeck, format, rules],
  )
  const switches = useMemo(() => ({ decks: many, list, detail }), [many, list, detail])
  const api = useMemo(
    () => lobbyApi(game.session, { format, lookup: (id) => deckFacts(id, switches) }),
    [game, format, switches],
  )
  const decks = useMemo(() => lobbyDecks(switches), [switches])
  const precons = useMemo(() => lobbyPrecons(switches), [switches])
  const auth = useMemo(
    () =>
      createAuthStore({
        // The signed-in page asks for its games, which an account-less client would answer by ending the sign-in.
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
      <GameContext value={game}>
        <p className="sr-only" data-fixture={LOBBY_FIXTURE_MARKER}>
          Development lobby with {seats} seats
        </p>
        <Lobby api={api} decks={decks} precons={precons} token={undefined} />
      </GameContext>
    </AuthContext>
  )
}
