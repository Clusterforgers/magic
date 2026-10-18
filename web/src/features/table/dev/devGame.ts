/** A game around a fixture, with no server: a store holding the fixture state and a session that only chats locally. */
import type { GameContextValue } from '@/features/game/gameContext'
import type { GameLookup } from '@/features/game/useGameInfo'
import { previewKey } from '@/play/preview'
import type { Session } from '@/play/session'
import { appendChat, createPlayStore, initialPlayState } from '@/play/store'
import type { PendingDecision, PlayState, PlayStore } from '@/play/store'
import { indexById } from '@/play/view'
import { devPreview } from './devForecast'
import { buildFixture } from './fixture'
import type { Fixture, FixtureOptions } from './fixture'
import { DEV_PROMPTS, DEV_TABLES } from './prompts'

/**
 * The play state of a seat that has just seen `fixture`. It faces `pending`, by default a priority decision with
 * nothing to play and nothing to tap, so a pass is all there is.
 */
export function fixtureState(fixture: Fixture, pending?: PendingDecision | null): PlayState {
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

/** How long the dev table takes to answer a preview, standing in for the network and the engine. */
const PREVIEW_LATENCY_MS = 120

/** How a fixture session treats previews. */
export interface FixtureSessionOptions {
  /**
   * Milliseconds before a preview is answered, with the forecast `devPreview` works out. Left out, previews are
   * refused the way a socket that is not open refuses them.
   */
  previewAfter?: number
}

/**
 * A session over `store`: nothing connects, a chat line is added to the store, and an answer is taken as the real
 * session takes it (the pending decision is cleared) but goes nowhere; with no decision pending it is refused.
 */
export function fixtureSession(store: PlayStore, options: FixtureSessionOptions = {}): Session {
  // As in the real session, only the newest preview is answered, and only while its decision is the pending one.
  let newest = 0
  return {
    store,
    connect: () => undefined,
    disconnect: () => undefined,
    ready: () => false,
    answer() {
      if (store.getState().pending === null) return false
      store.setState({ pending: null, preview: null })
      return true
    },
    preview(decision, answer) {
      const { previewAfter } = options
      const { pending, view } = store.getState()
      if (previewAfter === undefined || view === null || pending?.id !== decision) return false
      newest += 1
      const mine = newest
      const forecast = devPreview(view, answer)
      window.setTimeout(() => {
        if (mine !== newest || store.getState().pending?.id !== decision) return
        store.setState({ preview: { decision, key: previewKey(answer), forecast } })
      }, previewAfter)
      return true
    },
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

/**
 * Options of a fixture game: the table, the name in `DEV_PROMPTS` of the decision to face (none by default) and
 * `turn: 'you'` for the viewer's own turn (it overrides `active`).
 */
export interface FixtureGameOptions extends FixtureOptions {
  prompt?: string
  turn?: 'you'
  /** Milliseconds the table takes to answer a preview; `PREVIEW_LATENCY_MS` when left out. */
  previewMs?: number
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
  // The dev table answers a preview after a moment, so the "checking outcome" state can be looked at too.
  const session = fixtureSession(store, { previewAfter: options.previewMs ?? PREVIEW_LATENCY_MS })
  return { game: 'dev-fixture', session, lookup: fixtureLookup(fixture) }
}
