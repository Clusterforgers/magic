/** A game around a fixture, with no server: a store holding the fixture state and a session that only chats locally. */
import type { GameContextValue } from '@/features/game/gameContext'
import type { GameLookup } from '@/features/game/useGameInfo'
import type { Session } from '@/play/session'
import { appendChat, createPlayStore, initialPlayState } from '@/play/store'
import type { PlayState, PlayStore } from '@/play/store'
import { indexById } from '@/play/view'
import { buildFixture } from './fixture'
import type { Fixture, FixtureOptions } from './fixture'

/** The play state of a seat that has just seen `fixture`, with a priority decision waiting for it. */
export function fixtureState(fixture: Fixture): PlayState {
  const { view, log, chat } = fixture
  const options = { playable_lands: [], castable: [], mana_abilities: [], can_pass: true }
  return {
    ...initialPlayState(),
    status: 'open',
    seat: view.viewer,
    seatCount: view.players.length,
    view,
    objectsById: indexById(view.objects),
    seq: log.length,
    pending: { id: 1, decision: { player: view.viewer, kind: { Priority: options } } },
    log,
    chat,
    phase: 'playing',
  }
}

/** What REST would say about the fixture's game: every seat named and connected. */
export function fixtureLookup(fixture: Fixture): GameLookup {
  const seats = fixture.names.map((name, seat) => ({ seat, name }))
  const connected = seats.map((entry) => entry.seat)
  return { game: { started: true, seats, connected, result: null }, missing: false, failed: false }
}

/** A session over `store`: nothing connects, answers are refused, and a chat line is added to the store. */
export function fixtureSession(store: PlayStore): Session {
  return {
    store,
    connect: () => undefined,
    disconnect: () => undefined,
    ready: () => false,
    answer: () => false,
    concede: () => false,
    chat(text) {
      const line = text.trim()
      if (line === '') return false
      store.setState((state) => ({ chat: appendChat(state.chat, { from: state.seat ?? 0, text: line }) }))
      return true
    },
    resync: () => false,
    dismissNotice: () => store.setState({ notice: null }),
  }
}

/** A game context holding a fresh fixture table. */
export function fixtureGame(options: FixtureOptions): GameContextValue {
  const fixture = buildFixture(options)
  const store = createPlayStore()
  store.setState(fixtureState(fixture))
  return { game: 'dev-fixture', session: fixtureSession(store), lookup: fixtureLookup(fixture) }
}
