/**
 * One seat's play session: owns a `GameSocket`, applies the client rule of docs/protocol.md to what it hears and
 * keeps the result in a zustand store. No React; a screen reads `store` and calls the actions.
 */
import { GameSocket } from '@/net/socket'
import type { GameSocketOptions, SocketEvent } from '@/net/socket'
import { browserTimers } from '@/net/timers'
import type { Answer, DecisionId, ServerMessage, YieldUntil } from '@/protocol/generated'
import { previewKey } from './preview'
import { appendChat, appendLog, createPlayStore, dropLogFrom } from './store'
import type { PlayState, PlayStore } from './store'
import { applyBatch, fromFirstBatch, fromSnapshot, indexById } from './view'

/** The longest chat line the server accepts, in characters (`MAX_CHAT_CHARS` in `crates/mtg-server/src/actor.rs`). */
export const MAX_CHAT_CHARS = 500

/** What a session needs: which game and seat to join, and optionally fakes for sockets and timers in tests. */
export type SessionDeps = Omit<GameSocketOptions, 'onEvent'>

/** A seat's connection and live state. An action returns true when its message went out or was queued under the rate limit. */
export interface Session {
  /** The live state; in React read it with `useStore(session.store, selector)`. */
  readonly store: PlayStore
  /** Starts connecting; safe to call twice (React StrictMode) and again after `replaced` or `stopped`. */
  connect(): void
  /** Closes the socket and stops retrying; the state stays as it is. */
  disconnect(): void
  /** Says this seat is ready to start; the game begins once every seat has. */
  ready(): boolean
  /** Takes `ready()` back; the server ignores it once the game has started. */
  notReady(): boolean
  /** Answers the pending decision, once; false when there is none or the socket is not open. */
  answer(answer: Answer): boolean
  /**
   * Asks the engine what combat damage would do if `answer` were given to the pending decision `decision`, without
   * giving it; the reply lands in `store.preview` (docs/protocol.md, Preview). Advice only: false when `decision` is
   * not the pending one, when the socket is not open and once the server has shown it does not know previews.
   */
  preview(decision: DecisionId, answer: Answer): boolean
  /** Gives up the game. */
  concede(): boolean
  /**
   * Asks the server to pass priority for this seat whenever it has nothing to do, or to stop doing so. The store's
   * `autoPass` follows once the message is out; false (and no change) when the socket is not open.
   */
  setAutoPass(enabled: boolean): boolean
  /**
   * Asks the server to pass priority for this seat until `until`, so it is not asked again until something happens
   * (see `ClientMessage::Yield`). The store's `yielding` follows when the server says so, not before, and the
   * pending priority decision goes with it; false when the socket is not open.
   */
  yieldUntil(until: YieldUntil): boolean
  /** Asks the server to end this seat's yield; `yielding` becomes null when it confirms. False when the socket is not open. */
  cancelYield(): boolean
  /** Says `text` to the table (trimmed, at most `MAX_CHAT_CHARS` characters). */
  chat(text: string): boolean
  /** Asks the server for a fresh snapshot; the escape hatch when the game looks stuck. */
  resync(): boolean
  /** Clears the notice. */
  dismissNotice(): void
}

type Of<T extends ServerMessage['type']> = Extract<ServerMessage, { type: T }>
type StatusEvent = Extract<SocketEvent, { type: 'status' }>

const NOTICES = {
  unreadable: 'Ignored a message from the server that this page could not read.',
  stale: 'That decision was no longer pending, so your answer was not used. The game was refreshed.',
  internal:
    'The server hit an internal error and your last action may not have counted. Use resync if the game looks stuck.',
  illegal: (detail: string) => `That answer is not allowed: ${detail}`,
  refused: (detail: string) => `The server did not accept that: ${detail}`,
  tooLong: `Chat lines are at most ${MAX_CHAT_CHARS} characters.`,
}

/** Creates a session; it does nothing until `connect()`. */
export function createSession(deps: SessionDeps): Session {
  const store = createPlayStore()
  const { getState, setState } = store
  // Bookkeeping of the current connection, not state a screen needs.
  let awaitingSnapshot = false
  let lastAnswered: number | null = null
  // The previews asked for and not yet answered, oldest first: the server answers each one, in order.
  let asked: { decision: DecisionId; key: string }[] = []
  // False once an older server has refused a Preview as a message it does not know.
  let previewsKnown = true
  const socket = new GameSocket({ ...deps, onEvent })
  const timers = deps.timers ?? browserTimers
  // The one timer that keeps `clock` in step with the pending decision; cancelled whenever that decision goes.
  let clockTimer: number | null = null
  store.subscribe((state, previous) => {
    if (state.pending !== previous.pending) syncClock(state.pending)
  })

  /** Starts, restarts or stops the clock for `pending`, counting from now: the moment its message arrived. */
  function syncClock(pending: PlayState['pending']): void {
    if (clockTimer !== null) timers.clearTimeout(clockTimer)
    clockTimer = null
    if (pending === null || pending.deadlineMs === undefined) {
      if (getState().clock !== null) setState({ clock: null })
      return
    }
    const { id, deadlineMs: totalMs } = pending
    const endsAt = timers.now() + totalMs
    const tick = (): void => {
      clockTimer = null
      const leftMs = endsAt - timers.now()
      if (leftMs <= 0) {
        // The server has answered for this seat, or is about to; either way this prompt is over.
        if (getState().pending?.id === id) setState({ pending: null, preview: null })
        return
      }
      setState({ clock: { id, totalMs, leftMs } })
      // Wake when the whole seconds left change: for 9.5 s that is in half a second, for 9 s in a full one. A timer
      // that fires a hair early finds itself just above the boundary and asks for the hair that is left.
      clockTimer = timers.setTimeout(tick, leftMs - 1000 * (Math.ceil(leftMs / 1000) - 1))
    }
    tick()
  }

  function onEvent(event: SocketEvent): void {
    switch (event.type) {
      case 'status':
        onStatus(event)
        return
      case 'message':
        onMessage(event.message)
        return
      case 'frame_error':
        setState({ notice: NOTICES.unreadable })
        return
    }
  }

  function onStatus(event: StatusEvent): void {
    const live = event.status === 'connecting' || event.status === 'open'
    if (!live) {
      // A decision, its id and a pending resync belong to one connection; the next Snapshot starts fresh.
      awaitingSnapshot = false
      lastAnswered = null
      asked = []
    }
    setState((state) => ({
      status: event.status,
      attempt: event.attempt,
      notice: event.reason ?? (event.status === 'connecting' ? null : state.notice),
      pending: live ? state.pending : null,
      // Only a live connection hears of the lobby; once it is gone the roster is no longer the server's latest word.
      lobby: live ? state.lobby : null,
      preview: live ? state.preview : null,
    }))
  }

  function onMessage(message: ServerMessage): void {
    switch (message.type) {
      case 'Welcome':
        awaitingSnapshot = false
        lastAnswered = null
        asked = []
        previewsKnown = true
        setState({
          seat: message.player,
          seatCount: message.seats,
          autoPass: message.auto_pass,
          yielding: message.yielding,
        })
        return
      case 'Yielding':
        onYielding(message)
        return
      case 'Lobby':
        // The server sends the whole roster each time, so the newest one simply replaces the last.
        setState({ lobby: { seats: message.seats, needed: message.needed } })
        return
      case 'Snapshot':
        onSnapshot(message)
        return
      case 'Events':
        onEvents(message)
        return
      case 'Decision':
        onDecision(message)
        return
      case 'Forecast':
        onForecast(message)
        return
      case 'Chat':
        setState((state) => ({ chat: appendChat(state.chat, { from: message.from, text: message.text }) }))
        return
      case 'Error':
        onError(message)
        return
      case 'GameOver':
        socket.retire()
        setState({ result: message.result, phase: 'over', pending: null, preview: null, yielding: null })
        return
      default: {
        // Compile-time exhaustiveness: a new `ServerMessage` variant stops building here until it is handled.
        const unhandled: never = message
        void unhandled
      }
    }
  }

  function onSnapshot(message: Of<'Snapshot'>): void {
    awaitingSnapshot = false
    lastAnswered = null
    setState((state) => {
      const view = fromSnapshot(state.view, message.state)
      return {
        view,
        objectsById: view.objects === state.view?.objects ? state.objectsById : indexById(view.objects),
        seq: message.seq,
        pending: null, // the server sends the current decision right after the snapshot
        preview: null,
        log: dropLogFrom(state.log, message.seq),
        phase: state.result === null ? 'playing' : 'over',
        lobby: null, // the game is on, so the roster of the table before it is over
      }
    })
  }

  function onEvents(batch: Of<'Events'>): void {
    if (awaitingSnapshot) return
    const state = getState()
    const view =
      state.view === null
        ? fromFirstBatch(batch)
        : batch.from_seq === state.seq
          ? applyBatch(state.view, batch)
          : null
    if (view === null) {
      // Wait for the Snapshot only if the Resync really went out.
      awaitingSnapshot = socket.send({ type: 'Resync' })
      return
    }
    setState({
      view,
      objectsById: view.objects === state.view?.objects ? state.objectsById : indexById(view.objects),
      seq: batch.from_seq + batch.events.length,
      log: appendLog(state.log, batch.from_seq, batch.events),
      phase: state.result === null ? 'playing' : 'over',
      lobby: null,
    })
  }

  function onDecision(message: Of<'Decision'>): void {
    // After a gap the server repeats the pending decision behind the snapshot it sends; before that the ids may be unknown.
    if (awaitingSnapshot || message.id === lastAnswered) return
    setState({
      pending:
        message.deadline_ms === undefined
          ? { id: message.id, decision: message.decision }
          : { id: message.id, decision: message.decision, deadlineMs: message.deadline_ms },
      preview: null,
    })
  }

  function onYielding(message: Of<'Yielding'>): void {
    // A yield that starts has passed for the priority decision on screen (or is about to), so that prompt is over.
    // Other decisions stay: the server still asks for attackers, blockers and the like.
    setState((state) => {
      const passed =
        message.until !== null && state.pending !== null && 'Priority' in state.pending.decision.kind
      return passed ? { yielding: message.until, pending: null, preview: null } : { yielding: message.until }
    })
  }

  function onForecast(message: Of<'Forecast'>): void {
    const request = asked.shift()
    // The server answers every preview, in order. Only the answer to the newest one says anything about the choice
    // on screen, and only while its decision is the pending one.
    if (request === undefined || asked.length > 0) return
    const { pending } = getState()
    if (pending === null || pending.id !== message.decision || request.decision !== message.decision) return
    setState({ preview: { decision: message.decision, key: request.key, forecast: message.forecast } })
  }

  function onError(message: Of<'Error'>): void {
    switch (message.code) {
      case 'StaleDecision':
        setState({ notice: NOTICES.stale })
        resyncUnlessWaiting()
        return
      case 'IllegalAnswer':
        setState({ notice: NOTICES.illegal(message.message) })
        resyncUnlessWaiting()
        return
      case 'BadMessage':
        // A server that predates previews refuses each Preview as a message it does not know. That is no news for
        // the player: stop asking and let the prompts say the outcome shows after confirming.
        if (asked.shift() !== undefined) {
          previewsKnown = false
          return
        }
        setState({ notice: NOTICES.refused(message.message) })
        return
      case 'Internal':
        setState({ notice: NOTICES.internal })
        return
      case 'UnknownGame':
      case 'UnsupportedVersion':
        return // the socket stops itself and reports why through its status
    }
  }

  function resyncUnlessWaiting(): void {
    if (!awaitingSnapshot) socket.send({ type: 'Resync' })
  }

  return {
    store,
    connect: () => socket.connect(),
    disconnect: () => socket.disconnect(),
    ready: () => socket.send({ type: 'Ready' }),
    notReady: () => socket.send({ type: 'NotReady' }),
    answer(answer) {
      const { pending } = getState()
      if (pending === null || !socket.send({ type: 'Answer', decision: pending.id, answer })) return false
      lastAnswered = pending.id
      setState({ pending: null, preview: null })
      return true
    },
    preview(decision, answer) {
      const { pending } = getState()
      if (!previewsKnown || pending === null || pending.id !== decision) return false
      if (!socket.send({ type: 'Preview', decision, answer })) return false
      asked.push({ decision, key: previewKey(answer) })
      return true
    },
    concede: () => socket.send({ type: 'Concede' }),
    setAutoPass(enabled) {
      if (!socket.send({ type: 'AutoPass', enabled })) return false
      setState({ autoPass: enabled })
      return true
    },
    yieldUntil: (until) => socket.send({ type: 'Yield', until }),
    cancelYield: () => socket.send({ type: 'CancelYield' }),
    chat(text) {
      const line = text.trim()
      if (line === '') return false
      if ([...line].length > MAX_CHAT_CHARS) {
        setState({ notice: NOTICES.tooLong })
        return false
      }
      return socket.send({ type: 'Chat', text: line })
    },
    resync: () => socket.send({ type: 'Resync' }),
    dismissNotice: () => setState({ notice: null }),
  }
}
