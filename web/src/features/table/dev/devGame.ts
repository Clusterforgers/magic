/** A game around a fixture, with no server: a store holding the fixture state and a session that only chats locally. */
import type { GameContextValue } from '@/features/game/gameContext'
import type { GameLookup } from '@/features/game/useGameInfo'
import { previewKey } from '@/play/preview'
import type { Session } from '@/play/session'
import { canHoldStop } from '@/play/stops'
import { appendChat, createPlayStore, initialPlayState } from '@/play/store'
import type { PendingDecision, PlayState, PlayStore } from '@/play/store'
import { indexById } from '@/play/view'
import type { FullControlMode, HouseRules, Step, Stop } from '@/protocol/generated'
import { devPreview } from './devForecast'
import { withStop } from './devStops'
import { withRealCards } from './realCards'
import type { RealIds } from './realCards'
import { buildFixture } from './fixture'
import type { Fixture, FixtureOptions } from './fixture'
import { DEV_PROMPTS, DEV_TABLES, DEV_YIELDS } from './prompts'
import { STATES_BASE_PERMANENTS, withStates } from './states'
import { withEquip } from './equip'

/**
 * The play state of a seat that has just seen `fixture`. It faces `pending`, by default a priority decision with
 * nothing to play and nothing to tap, so a pass is all there is.
 */
export function fixtureState(fixture: Fixture, pending?: PendingDecision | null): PlayState {
  const { view, log, chat } = fixture
  const options = { playable_lands: [], castable: [], mana_abilities: [], can_act: false, can_pass: true }
  const shown: PendingDecision | null =
    pending === undefined
      ? { id: 1, decision: { player: view.viewer, kind: { Priority: options } } }
      : pending
  return {
    ...initialPlayState(),
    status: 'open',
    seat: view.viewer,
    seatCount: view.players.length,
    view,
    objectsById: indexById(view.objects),
    seq: log.length,
    pending: shown,
    // A decision on a clock shows its full time; nothing ticks it here, the bar's own animation is all that moves.
    clock:
      shown?.deadlineMs === undefined
        ? null
        : { id: shown.id, totalMs: shown.deadlineMs, leftMs: shown.deadlineMs },
    log,
    chat,
    phase: 'playing',
  }
}

/** What REST would say about the fixture's game: every seat named and connected. */
export function fixtureLookup(fixture: Fixture): GameLookup {
  const seats = fixture.names.map((name, seat) => ({ seat, name }))
  const connected = seats.map((entry) => entry.seat)
  // A game that runs has every seat ready.
  const game = { started: true, seats, connected, ready: connected, result: null }
  return { game, missing: false, failed: false }
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
    notReady: () => false,
    answer() {
      if (store.getState().pending === null) return false
      store.setState({ pending: null, preview: null, clock: null })
      return true
    },
    // A manual action changes nothing here but the log, and whatever decision waits stays, as the server has it.
    manual(action) {
      const { log, seat } = store.getState()
      const event = { Public: { ManualAction: { player: seat ?? 0, action } } } as const
      store.setState({ log: [...log, { seq: (log.at(-1)?.seq ?? -1) + 1, event }] })
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
    // As the server answers a stop: it holds it and says the whole list (untap and cleanup it refuses).
    setStop(side, step, on, keep) {
      if (!canHoldStop(step)) return false
      store.setState((state) => ({ stops: withStop(state.stops, side, step, on, keep) }))
      return true
    },
    // As the server answers Full Control: turning it on ends a running yield.
    setFullControl(mode) {
      store.setState((state) => ({ fullControl: mode, yielding: mode === 'Off' ? state.yielding : null }))
      return true
    },
    // As the server answers a yield (it refuses one under Full Control): it starts, and the priority decision on screen is passed.
    yieldUntil(until) {
      if (store.getState().fullControl !== 'Off') return false
      store.setState((state) => ({
        yielding: until,
        pending: state.pending !== null && 'Priority' in state.pending.decision.kind ? null : state.pending,
        clock: null,
      }))
      return true
    },
    cancelYield() {
      store.setState({ yielding: null })
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
  /** Oracle ids by name (`?cards=real`): the cards get identities and the showcase joins the viewer's battlefield. */
  real?: RealIds
  prompt?: string
  turn?: 'you'
  /** The step the turn is in, whatever the fixture or the prompt's moment has; theirs when left out. */
  step?: Step
  /** House rules that differ from the format's own, from the start; none when left out. */
  rules?: true
  /** A permanent for every state a card shows joins the table (`dev/states.ts`), on a table with fewer permanents. */
  states?: true
  /** The attachments scene joins the table (`dev/equip.ts`). */
  equip?: true
  /** Milliseconds the table takes to answer a preview; `PREVIEW_LATENCY_MS` when left out. */
  previewMs?: number
  /** Gives the viewer a plain priority decision (nothing to play) when no prompt is named, so manual actions can be taken. */
  holding?: true
  /** The stops the viewer holds from the start; none when left out. */
  stops?: Stop[]
  /** The Full Control mode the viewer holds from the start; off when left out. */
  fullControl?: FullControlMode
}

/** The house rules `rules` gives the fixture table: two differ from Commander's own and one is its own. */
const DEV_TABLE_RULES: HouseRules = {
  starting_life: 30,
  commander_tax: 0,
  open_hands: true,
  free_mulligans: 1,
}

/** A game context holding a fresh fixture table. */
export function fixtureGame(options: FixtureGameOptions): GameContextValue {
  const sized = options.states === true ? { ...options, permanents: STATES_BASE_PERMANENTS } : options
  const laid = buildFixture(sized.turn === 'you' ? { ...sized, active: sized.viewer ?? 0 } : sized)
  const stated = options.states === true ? withStates(laid) : laid
  const plain = options.equip === true ? withEquip(stated) : stated
  const built = options.real === undefined ? plain : withRealCards(plain, options.real, options.viewer ?? 0)
  const store = createPlayStore()
  const name = options.prompt ?? ''
  const prompt = Object.hasOwn(DEV_PROMPTS, name) ? DEV_PROMPTS[name] : undefined
  // A prompt may need another moment of the game than the fixture shows (the viewer's own turn, an attack to block),
  // and so may a state without any prompt, such as a seat that is already passing.
  const table = Object.hasOwn(DEV_TABLES, name) ? DEV_TABLES[name] : undefined
  const moment = table === undefined ? built : table(built)
  const stepped =
    options.step === undefined
      ? moment
      : { ...moment, view: { ...moment.view, turn: { ...moment.view.turn, step: options.step } } }
  const fixture =
    options.rules === true ? { ...stepped, view: { ...stepped.view, house_rules: DEV_TABLE_RULES } } : stepped
  const shown = prompt !== undefined ? prompt(fixture) : options.holding === true ? undefined : null
  store.setState(fixtureState(fixture, shown))
  if (options.stops !== undefined) store.setState({ stops: options.stops })
  if (options.fullControl !== undefined) store.setState({ fullControl: options.fullControl })
  // A seat that is already yielding faces no priority decision, only the line that says it is passing.
  if (Object.hasOwn(DEV_YIELDS, name)) store.setState({ yielding: DEV_YIELDS[name] ?? null })
  // The dev table answers a preview after a moment, so the "checking outcome" state can be looked at too.
  const session = fixtureSession(store, { previewAfter: options.previewMs ?? PREVIEW_LATENCY_MS })
  return { game: 'dev-fixture', session, lookup: fixtureLookup(fixture) }
}
