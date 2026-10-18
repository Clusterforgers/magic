/**
 * One seat's play session: owns a `GameSocket`, applies the client rule of docs/protocol.md to what it hears and
 * keeps the result in a zustand store. No React; a screen reads `store` and calls the actions.
 */
import { GameSocket } from '@/net/socket'
import type { GameSocketOptions, SocketEvent } from '@/net/socket'
import type { Answer, DecisionId, ServerMessage } from '@/protocol/generated'
import { previewKey } from './preview'
import { appendChat, appendLog, createPlayStore, dropLogFrom } from './store'
import type { PlayStore } from './store'
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
        setState({ seat: message.player, seatCount: message.seats })
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
        setState({ result: message.result, phase: 'over', pending: null, preview: null })
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
    })
  }

  function onDecision(message: Of<'Decision'>): void {
    // After a gap the server repeats the pending decision behind the snapshot it sends; before that the ids may be unknown.
    if (awaitingSnapshot || message.id === lastAnswered) return
    setState({ pending: { id: message.id, decision: message.decision }, preview: null })
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
