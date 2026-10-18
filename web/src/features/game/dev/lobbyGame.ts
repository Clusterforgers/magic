/** A lobby around a fixture, with no server: a store holding a roster and a session that answers Ready itself. */
import type { GameContextValue } from '@/features/game/gameContext'
import { createSession } from '@/play/session'
import type { Session } from '@/play/session'
import { initialPlayState } from '@/play/store'
import type { LobbyRoster } from '@/play/store'
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

/**
 * A session that plays the server's part for this seat: Ready and Not ready change the roster at once. It is a real
 * session that never connects, so it has every action the page may call, now and as the session grows.
 */
function lobbySession(me: number): Session {
  const inert = createSession({ game: 'dev-lobby', name: 'dev' })
  const { store } = inert
  const setReady = (ready: boolean): boolean => {
    store.setState((state) => {
      if (state.lobby === null) return {}
      const seats = state.lobby.seats.map((entry) => (entry.seat === me ? { ...entry, ready } : entry))
      return { lobby: { seats, needed: seats.filter((entry) => !entry.ready).length } }
    })
    return true
  }
  return { ...inert, ready: () => setReady(true), notReady: () => setReady(false) }
}

/** A game context holding a lobby: the roster of `options`, this page seated at `me`, the socket open. */
export function lobbyGame(options: DevLobbySearch): GameContextValue {
  const session = lobbySession(options.me)
  session.store.setState({
    ...initialPlayState(),
    status: 'open',
    seat: options.me,
    seatCount: options.seats,
    lobby: fixtureRoster(options),
  })
  return { game: 'dev-lobby', session, lookup: { game: null, missing: false, failed: false } }
}
