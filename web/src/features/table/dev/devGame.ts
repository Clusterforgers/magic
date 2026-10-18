/** A game around a fixture, with no server: a store holding the fixture state and a session that only chats locally. */
import type { GameContextValue } from '@/features/game/gameContext'
import type { GameLookup } from '@/features/game/useGameInfo'
import type { Session } from '@/play/session'
import { appendChat, createPlayStore, initialPlayState } from '@/play/store'
import type { PendingDecision, PlayState, PlayStore } from '@/play/store'
import { indexById } from '@/play/view'
import { buildFixture } from './fixture'
import type { Fixture, FixtureOptions } from './fixture'
import { DEV_PROMPTS, DEV_TABLES } from './prompts'

/**
 * The play state of a seat that has just seen `fixture`. It faces `pending`, by default a priority decision with
 * nothing to play and nothing to tap, so a pass is all there is.
 */
export function fixtureState(fixture: Fixture, pending?: PendingDecision | null): PlayState {
  const { view, log, chat } = fixture
  const options = { playable_lands: [], castable: [], mana_abilities: [], can_act: false, can_pass: true }
  return {
    ...initialPlayState(),
    status: 'open',
    seat: view.viewer,
    seatCount: view.players.length,
    view,
    objectsById: indexById(view.objects),
    seq: log.length,
    pending:
      pending === undefined
        ? { id: 1, decision: { player: view.viewer, kind: { Priority: options } } }
        : pending,
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

/**
 * A session over `store`: nothing connects, a chat line is added to the store, and an answer is taken as the real
 * session takes it (the pending decision is cleared) but goes nowhere; with no decision pending it is refused.
 */
export function fixtureSession(store: PlayStore): Session {
  return {
    store,
    connect: () => undefined,
    disconnect: () => undefined,
    ready: () => false,
    answer() {
      if (store.getState().pending === null) return false
      store.setState({ pending: null })
      return true
    },
    concede: () => false,
    setAutoPass(enabled) {
      store.setState({ autoPass: enabled })
      return true
    },
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

/**
 * Options of a fixture game: the table, the name in `DEV_PROMPTS` of the decision to face (none by default) and
 * `turn: 'you'` for the viewer's own turn (it overrides `active`).
 */
export interface FixtureGameOptions extends FixtureOptions {
  prompt?: string
  turn?: 'you'
}

/** A game context holding a fresh fixture table. */
export function fixtureGame(options: FixtureGameOptions): GameContextValue {
  const built = buildFixture(options.turn === 'you' ? { ...options, active: options.viewer ?? 0 } : options)
  const store = createPlayStore()
  const name = options.prompt ?? ''
  const prompt = Object.hasOwn(DEV_PROMPTS, name) ? DEV_PROMPTS[name] : undefined
  // A prompt may need another moment of the game than the fixture shows (the viewer's own turn, an attack to block).
  const table = prompt !== undefined && Object.hasOwn(DEV_TABLES, name) ? DEV_TABLES[name] : undefined
  const fixture = table === undefined ? built : table(built)
  store.setState(fixtureState(fixture, prompt === undefined ? null : prompt(fixture)))
  return { game: 'dev-fixture', session: fixtureSession(store), lookup: fixtureLookup(fixture) }
}
