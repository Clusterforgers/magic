/** A game around a fixture, with no server: a store holding the fixture state and a session that only chats locally. */
import type { GameContextValue } from '@/features/game/gameContext'
import type { GameLookup } from '@/features/game/useGameInfo'
import { previewKey } from '@/play/preview'
import type { Session } from '@/play/session'
import { canHoldStop } from '@/play/stops'
import { appendChat, appendLog, createPlayStore, initialPlayState } from '@/play/store'
import type { PendingDecision, PlayState, PlayStore } from '@/play/store'
import { indexById, withPresence } from '@/play/view'
import type {
  FullControlMode,
  HouseRules,
  ManaOption,
  ObjectId,
  ObjectView,
  PlayerId,
  StateView,
  Step,
  Stop,
} from '@/protocol/generated'
import { ruleKey } from '../autoSkips'
import { devPreview } from './devForecast'
import { devManaAnswer } from './devMana'
import { withStop } from './devStops'
import { withRealCards } from './realCards'
import { withReworkedBoss } from './reworkedBoss'
import type { DevRework } from './reworkedBoss'
import type { RealIds } from './realCards'
import { buildFixture } from './fixture'
import type { Fixture, FixtureOptions } from './fixture'
import { DEV_PROMPTS, DEV_TABLES, DEV_YIELDS } from './prompts'
import { STATES_BASE_PERMANENTS, withStates } from './states'
import { withCrowd } from './crowdScene'
import { withEquip } from './equip'
import { withRevealed } from './revealScene'
import { withPlayerCounters } from './countersScene'
import { withOneVoice } from './blockedScene'
import { detailRewound } from './detailScene'
import { endedWith, withTeams } from './endScene'
import type { DevResult, DevTeams } from '../devSearch'

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

/** The view with the life of `player` moved by `delta`, as the next state shows it after a `Life` action. */
function withLife(view: StateView, player: PlayerId, delta: number): StateView {
  return {
    ...view,
    players: view.players.map((one) => (one.seat === player ? { ...one, life: one.life + delta } : one)),
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
  const tapped = new Map<ObjectId, ManaOption>()
  return {
    store,
    connect: () => undefined,
    disconnect: () => undefined,
    ready: () => false,
    notReady: () => false,
    answer(given) {
      if (store.getState().pending === null) return false
      // Tapping for mana, and pressing the source again, play out as the server has them.
      const mana = devManaAnswer(store.getState(), given, tapped)
      if (mana !== null) {
        store.setState(mana)
        return true
      }
      store.setState({ pending: null, preview: null, clock: null })
      return true
    },
    // A manual action changes nothing here but the log (and life, which the counter shows), and whatever decision waits stays, as the server has it.
    manual(action, reply) {
      const { log, seat, view } = store.getState()
      const event = { Public: { ManualAction: { player: seat ?? 0, action } } } as const
      const moved =
        view !== null && 'Life' in action
          ? { view: withLife(view, action.Life.player, action.Life.delta) }
          : {}
      store.setState({ log: [...log, { seq: (log.at(-1)?.seq ?? -1) + 1, event }], ...moved })
      reply?.({ ok: true })
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
    // As the server answers: it holds the choice and says so.
    setEndTurnByHand(on) {
      store.setState({ endTurnByHand: on })
      return true
    },
    // As the server answers a yield to an ability: it keys it by the name and words on the stack and says the whole list.
    autoYield(object) {
      const shown = store.getState().objectsById[object]
      if (shown?.kind !== 'Ability' || shown.name === '') return false
      const key = { name: shown.name, text: shown.rules_text ?? '' }
      store.setState((state) => ({
        autoYields: state.autoYields.some((held) => held.name === key.name && held.text === key.text)
          ? state.autoYields
          : [...state.autoYields, key],
      }))
      return true
    },
    clearAutoYield(ability) {
      store.setState((state) => ({
        autoYields: state.autoYields.filter(
          (held) => held.name !== ability.name || held.text !== ability.text,
        ),
      }))
      return true
    },
    // As the server holds the rules: one per key, the newest in place of the last.
    autoSkip(rule) {
      store.setState((state) => ({
        autoSkips: [...state.autoSkips.filter((held) => ruleKey(held) !== ruleKey(rule)), rule],
      }))
      return true
    },
    cancelAutoSkip(rule) {
      store.setState((state) => ({
        autoSkips: state.autoSkips.filter((held) => ruleKey(held) !== ruleKey(rule)),
      }))
      return true
    },
    setAutoChoices(choices) {
      store.setState({ autoChoices: choices })
      return true
    },
    // The dev table has no passing to interrupt: the card is only marked as queued.
    castWhenAsked(card) {
      store.setState({ castWhenAsked: card })
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
    takeBack: () => false,
    voteTakeBack(approve) {
      store.setState((state) =>
        state.takeBackRequest === null
          ? {}
          : {
              takeBackRequest: null,
              takeBackNotice: {
                offer: state.takeBackRequest.offer,
                outcome: approve ? { type: 'Approved' } : { type: 'Declined', by: state.seat ?? 0 },
              },
            },
      )
      return true
    },
    dismissTakeBackNotice: () => store.setState({ takeBackNotice: null }),
    goBack() {
      // The dev table goes back at once: the offer is spent and the step is the one it named.
      store.setState({ goBackOffer: null, goBackRefused: null })
      return true
    },
    dismissGoBackRefusal: () => store.setState({ goBackRefused: null }),
    chat(text) {
      const line = text.trim()
      if (line === '') return false
      store.setState((state) => ({ chat: appendChat(state.chat, { from: state.seat ?? 0, text: line }) }))
      return true
    },
    resync: () => false,
    dismissNotice: () => store.setState({ notice: null }),
    clearDueLines: () => store.setState({ dueLines: [] }),
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
  turn?: 'you' | 'left'
  /** The step the turn is in, whatever the fixture or the prompt's moment has; theirs when left out. */
  step?: Step
  /** House rules that differ from the format's own, from the start; none when left out. */
  rules?: true
  /** A fight against a boss: the last seats are the boss and this many minions, a team against the others. */
  boss?: number
  /** The viewer is a fallen hero's Spirit with no permanents, hand or commander (`withSpirit`), in a boss fight. */
  spirit?: true
  /** With `boss`, how far the fight has come (`withReworkedBoss`): 1 before Phase 1 (the default), 2 with Phase 1 in force. */
  rework?: DevRework
  /** No seat has a permanent (`withBareBoards`): the start of a fight. */
  bare?: true
  /** Every opponent but the last out of the game, the last with an empty board. */
  alone?: true
  /** A permanent for every state a card shows joins the table (`dev/states.ts`), on a table with fewer permanents. */
  states?: true
  /** The attachments scene joins the table (`dev/equip.ts`). */
  equip?: true
  /** The crowded stack and the big attack join the table (`dev/crowdScene.ts`). */
  crowd?: true
  /** The first opponent's revealed hand cards and top card, a revealed card of the viewer's and two notes join the table (`dev/revealScene.ts`). */
  reveal?: true
  /** Counters on every player join the table (`dev/countersScene.ts`). */
  counters?: true
  /** Milliseconds the table takes to answer a preview; `PREVIEW_LATENCY_MS` when left out. */
  previewMs?: number
  /** Gives the viewer a plain priority decision (nothing to play) when no prompt is named, so manual actions can be taken. */
  holding?: true
  /** The stops the viewer holds from the start; none when left out. */
  stops?: Stop[]
  /** The Full Control mode the viewer holds from the start; off when left out. */
  fullControl?: FullControlMode
  /** A seat whose client is gone, with 12 s before the game acts for it, for the away mark and note. */
  away?: PlayerId
  /** Two teams, even seats against odd ones (`dev/endScene.ts`). */
  teams?: DevTeams
  /** The game is over with this result. */
  result?: DevResult
}

/** The house rules `rules` gives the fixture table: two differ from Commander's own and one is its own. */
const DEV_TABLE_RULES: HouseRules = {
  starting_life: 30,
  commander_tax: 0,
  open_hands: true,
  free_mulligans: 1,
}

/**
 * The table as a fight against a boss: the last seat but `minions` is the boss with its parts at `stage`
 * (`withReworkedBoss`); it and the seats after it are one team, every other seat the other.
 */
export function withBoss(moment: Fixture, minions: number, stage: DevRework = 1): Fixture {
  const count = moment.view.players.length
  const boss = Math.max(1, count - 1 - Math.max(0, minions))
  const players = moment.view.players.map((player) =>
    player.seat === boss
      ? {
          ...player,
          life: 120,
          boss: { phases: [], begun: 0 },
        }
      : player,
  )
  const seats = players.map((player) => (player.seat >= boss ? 1 : 0))
  const house_rules = { ...moment.view.house_rules, teams: { mode: 'teams' as const, seats } }
  return withReworkedBoss({ ...moment, view: { ...moment.view, players, house_rules } }, stage)
}

/** A table at the start of a fight: no seat has a permanent and nothing is attacking. */
export function withBareBoards(moment: Fixture): Fixture {
  const gone = new Set(
    moment.view.objects.filter((object) => object.zone === 'Battlefield').map((object) => object.id),
  )
  return {
    ...moment,
    view: {
      ...moment.view,
      objects: moment.view.objects.filter((object) => !gone.has(object.id)),
      battlefield: { count: 0, cards: [] },
      combat: { attackers: [], attackers_declared: false, blockers_declared: [] },
    },
  }
}

/**
 * The viewer as a fallen hero's Spirit: 0 life, no permanents, no hand and no commander, so the table has nothing of
 * the viewer's to draw but its pod.
 */
export function withSpirit(moment: Fixture): Fixture {
  const viewer = moment.view.viewer
  const mine = (object: ObjectView): boolean =>
    object.owner === viewer &&
    (object.zone === 'Battlefield' || (typeof object.zone === 'object' && 'Hand' in object.zone))
  const gone = new Set(moment.view.objects.filter(mine).map((object) => object.id))
  const players = moment.view.players.map((player) =>
    player.seat === viewer
      ? {
          ...player,
          life: 0,
          spirit: true,
          hand: { count: 0, cards: [] },
          command: { count: 0, cards: [] },
          commanders: [],
        }
      : player,
  )
  const left = (moment.view.battlefield.cards ?? []).filter((id) => !gone.has(id))
  return {
    ...moment,
    view: {
      ...moment.view,
      players,
      objects: moment.view.objects.filter((object) => !gone.has(object.id)),
      battlefield: { count: left.length, cards: left },
    },
  }
}

/**
 * A boss fight in which the boss's side has attacked: the boss with one creature per hero and each minion with one,
 * dealt to the heroes in turn from seat 0 so that every hero is attacked, nobody has blocked yet and it is the boss's
 * turn. A table without a boss is left as it is.
 */
export function withBossAttack(moment: Fixture): Fixture {
  const boss = moment.view.players.findIndex((player) => player.boss !== undefined)
  if (boss < 1) return moment
  const heroes = boss
  const fighter = (seat: PlayerId) =>
    moment.view.objects.filter(
      (object) =>
        object.zone === 'Battlefield' &&
        object.controller === seat &&
        object.types.includes('Creature') &&
        object.permanent?.summoning_sick === false,
    )
  const attackers: ObjectView[] = []
  for (let seat = boss; seat < moment.view.players.length; seat += 1) {
    attackers.push(...fighter(seat).slice(0, seat === boss ? heroes : 1))
  }
  const ids = new Set(attackers.map((object) => object.id))
  const was = new Set((moment.view.combat?.attackers ?? []).map((attacker) => attacker.object))
  const objects = moment.view.objects.map((object) =>
    object.permanent !== null && (ids.has(object.id) || was.has(object.id))
      ? { ...object, permanent: { ...object.permanent, tapped: ids.has(object.id) } }
      : object,
  )
  const combat = {
    attackers: attackers.map((object, at) => ({
      object: object.id,
      target: { Player: at % heroes },
      blocked: false,
      blockers: [],
    })),
    attackers_declared: true,
    blockers_declared: [],
  }
  const turn = { ...moment.view.turn, active: boss, step: 'DeclareBlockers' as const }
  return {
    ...moment,
    view: { ...moment.view, objects, combat, turn, priority: { holder: moment.view.viewer, passed: [boss] } },
  }
}

/**
 * Every opponent but the last out of the game, by turns conceded and out of life, their permanents gone with them, and
 * the last opponent with no permanents: the end of a free-for-all, where one empty board faces the viewer's.
 */
export function withLastStanding(moment: Fixture): Fixture {
  const viewer = moment.view.viewer
  const rivals = moment.view.players.filter((player) => player.seat !== viewer).map((player) => player.seat)
  const out = new Set(rivals.slice(0, -1))
  const emptied = new Set(rivals)
  const gone = new Set(
    moment.view.objects
      .filter((object) => object.zone === 'Battlefield' && emptied.has(object.controller))
      .map((object) => object.id),
  )
  let lost = 0
  const players = moment.view.players.map((player) => {
    if (!out.has(player.seat)) return player
    lost += 1
    return lost % 2 === 1
      ? { ...player, lost: 'Conceded' as const }
      : { ...player, life: 0, lost: 'ZeroLife' as const }
  })
  const left = (moment.view.battlefield.cards ?? []).filter((id) => !gone.has(id))
  return {
    ...moment,
    view: {
      ...moment.view,
      players,
      objects: moment.view.objects.filter((object) => !gone.has(object.id)),
      battlefield: { count: left.length, cards: left },
      combat: { attackers: [], attackers_declared: false, blockers_declared: [] },
    },
  }
}

/** The table with its active player gone (CR 800.4): the seat is out of the game and the turn goes on without it. */
function withActiveLeft(moment: Fixture): Fixture {
  const gone = moment.view.turn.active
  const players = moment.view.players.map((player) =>
    player.seat === gone ? { ...player, lost: 'Conceded' as const } : player,
  )
  return { ...moment, view: { ...moment.view, players, turn: { ...moment.view.turn, active_left: true } } }
}

/** A game context holding a fresh fixture table. */
export function fixtureGame(options: FixtureGameOptions): GameContextValue {
  const sized = options.states === true ? { ...options, permanents: STATES_BASE_PERMANENTS } : options
  const laid = buildFixture(sized.turn === 'you' ? { ...sized, active: sized.viewer ?? 0 } : sized)
  const stated = options.states === true ? withStates(laid) : laid
  const equipped = options.equip === true ? withEquip(stated) : stated
  const unveiled = options.reveal === true ? withRevealed(equipped) : equipped
  const revealed = options.counters === true ? withPlayerCounters(unveiled) : unveiled
  const plain = options.crowd === true ? withCrowd(revealed) : revealed
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
  const ruled =
    options.rules === true ? { ...stepped, view: { ...stepped.view, house_rules: DEV_TABLE_RULES } } : stepped
  const teamed = options.teams === undefined ? ruled : withTeams(ruled, options.teams)
  // A boss fight shows the boss's attack unless a prompt brings a moment of its own.
  const bossed =
    options.boss === undefined
      ? teamed
      : prompt === undefined && table === undefined
        ? withBossAttack(withBoss(teamed, options.boss, options.rework))
        : withBoss(teamed, options.boss, options.rework)
  const spirited = options.spirit === true && options.boss !== undefined ? withSpirit(bossed) : bossed
  const bared = options.bare === true ? withBareBoards(spirited) : spirited
  const lone = options.alone === true ? withLastStanding(bared) : bared
  const voiced = name === 'priority-blocked' ? withOneVoice(lone) : lone
  const fixture = options.turn === 'left' ? withActiveLeft(voiced) : voiced
  const shown = prompt !== undefined ? prompt(fixture) : options.holding === true ? undefined : null
  store.setState(fixtureState(fixture, shown))
  const rewound = name === 'priority-detail' ? detailRewound(fixture) : null
  if (rewound !== null)
    store.setState((state) => ({ log: appendLog(state.log, state.log.length, [rewound]) }))
  if (options.result !== undefined) {
    const { players, result } = endedWith(fixture, options.result)
    store.setState((state) => ({
      phase: 'over',
      result,
      pending: null,
      view: state.view === null ? null : { ...state.view, players },
    }))
  }
  if (options.stops !== undefined) store.setState({ stops: options.stops })
  if (options.fullControl !== undefined) store.setState({ fullControl: options.fullControl })
  const away = options.away
  if (away !== undefined)
    store.setState((state) => ({
      view: state.view === null ? null : withPresence(state.view, away, false),
      away: { [away]: 12_000 },
    }))
  if (name === 'priority-byhand') {
    const spell = fixture.view.objects.find((o) => o.zone === 'Stack' && o.controller === fixture.view.viewer)
    store.setState({ byHand: spell?.name ?? 'Your card' })
  }
  // A seat that is already yielding faces no priority decision, only the line that says it is passing.
  if (Object.hasOwn(DEV_YIELDS, name)) store.setState({ yielding: DEV_YIELDS[name] ?? null })
  // The dev table answers a preview after a moment, so the "checking outcome" state can be looked at too.
  const session = fixtureSession(store, { previewAfter: options.previewMs ?? PREVIEW_LATENCY_MS })
  return { game: 'dev-fixture', session, lookup: fixtureLookup(fixture) }
}
