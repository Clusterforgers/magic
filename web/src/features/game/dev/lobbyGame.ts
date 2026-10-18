/** A lobby around a fixture, with no server: a store holding a roster and a session that answers Ready itself. */
import type { GameContextValue } from '@/features/game/gameContext'
import type { Session } from '@/play/session'
import { createPlayStore, initialPlayState } from '@/play/store'
import type { LobbyRoster, PlayStore } from '@/play/store'
import type { DevLobbySearch } from './lobbySearch'

/** The text the dev page prints, so a production bundle can be searched for any trace of the fixture. */
export const LOBBY_FIXTURE_MARKER = 'dev-lobby-fixture-5b7d'

/** The names of the fixture's seats, in seat order. */
const SHORT = ['Ada', 'Bo', 'Cy', 'Dee', 'Eli', 'Fay', 'Gus', 'Hana', 'Ivo', 'Jun', 'Kit', 'Lena']

/** Names of the longest length the server accepts (32 characters), the first of them without a space to break at. */
const LONG = [
  'Wolfeschlegelsteinhausenbergerdo',
  'Bartholomew Featherstonehaugh Jr',
  'Maximiliana von Hohenzollern-Sig',
  'Ünïcödé Nämé Wïth Åccents Änd Öö',
  'Zebulon the Unpronounceable Ones',
  'Hubert Blaine Wolfeschlegelstein',
]

/** The roster `options` describe: everybody connected unless away, and the ready seats ready. */
export function fixtureRoster({ seats, ready, away, names }: DevLobbySearch): LobbyRoster {
  const listed = Array.from({ length: seats }, (_, seat) => ({
    seat,
    name: (names === 'long' ? LONG[seat % LONG.length] : SHORT[seat]) ?? `Seat ${seat + 1}`,
    connected: !away.includes(seat),
    ready: ready.includes(seat),
  }))
  return { seats: listed, needed: listed.filter((entry) => !entry.ready).length }
}

/** A session over `store` that plays the server's part for this seat: Ready and NotReady change the roster at once. */
function lobbySession(store: PlayStore, me: number): Session {
  const setReady = (ready: boolean): boolean => {
    store.setState((state) => {
      if (state.lobby === null) return {}
      const seats = state.lobby.seats.map((entry) => (entry.seat === me ? { ...entry, ready } : entry))
      return { lobby: { seats, needed: seats.filter((entry) => !entry.ready).length } }
    })
    return true
  }
  return {
    store,
    connect: () => undefined,
    disconnect: () => undefined,
    ready: () => setReady(true),
    notReady: () => setReady(false),
    answer: () => false,
    concede: () => false,
    setAutoPass: () => false,
    chat: () => false,
    resync: () => false,
    dismissNotice: () => undefined,
  }
}

/** A game context holding a lobby: the roster of `options`, this page seated at `me`, the socket open. */
export function lobbyGame(options: DevLobbySearch): GameContextValue {
  const store = createPlayStore()
  store.setState({
    ...initialPlayState(),
    status: 'open',
    seat: options.me,
    seatCount: options.seats,
    lobby: fixtureRoster(options),
  })
  return {
    game: 'dev-lobby',
    session: lobbySession(store, options.me),
    lookup: { game: null, missing: false, failed: false },
  }
}
