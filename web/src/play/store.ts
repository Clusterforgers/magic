/** The zustand store of one game session: everything a game screen reads, and nothing it writes. */
import { createStore } from 'zustand/vanilla'
import type { StoreApi } from 'zustand/vanilla'
import type { ConnectionStatus } from '@/net/socket'
import type {
  Decision,
  DecisionId,
  EventView,
  GameResult,
  LobbySeat,
  ObjectId,
  ObjectView,
  PlayerId,
  StateView,
} from '@/protocol/generated'

/** How many of the most recent events `log` keeps. */
export const LOG_LIMIT = 500
/** How many of the most recent chat lines `chat` keeps. */
export const CHAT_LIMIT = 200

/** Where the game is: `lobby` until a state arrives, `playing`, then `over` once the server says so. */
export type Phase = 'lobby' | 'playing' | 'over'

/** The decision waiting for this seat's answer; echo `id` in the answer. */
export interface PendingDecision {
  id: DecisionId
  decision: Decision
  /** Milliseconds the seat had when the server sent it, if the decision runs on a clock; see `PlayState.clock`. */
  deadlineMs?: number
}

/**
 * The response clock of the pending decision. When it runs out the server answers for the seat (a pass, or no
 * blocks) and the decision is gone; the session clears it at that moment, so a prompt is never left standing after
 * the server has moved on.
 */
export interface DecisionClock {
  /** The decision the clock belongs to. */
  id: DecisionId
  /** Milliseconds the seat had when the decision arrived (less than the server's full clock after a reconnect). */
  totalMs: number
  /** Milliseconds left at the last tick; the session ticks at every change of the whole seconds left. */
  leftMs: number
}

/** Who is at the table before the game starts, as the server last said it (`ServerMessage` `Lobby`). */
export interface LobbyRoster {
  /** Every seat in seat order: its name, whether a client is connected to it and whether it is ready. */
  seats: LobbySeat[]
  /** How many seats still have to press Ready. */
  needed: number
}

/** One event of the game log with its position in the server's log. */
export interface LogEntry {
  seq: number
  event: EventView
}

/** One line of table chat. */
export interface ChatLine {
  from: PlayerId
  text: string
}

/** Everything a game screen reads. Treat it as immutable: the session replaces what changes and nothing else. */
export interface PlayState {
  /** The connection; see `ConnectionStatus`. */
  status: ConnectionStatus
  /** Failed connection attempts in a row since the last Welcome; 0 while open. */
  attempt: number
  /** The seat this client plays, from the Welcome. */
  seat: PlayerId | null
  /** How many seats the table has. */
  seatCount: number | null
  /** What this seat may know of the game; null until a Snapshot or the first batch arrives. */
  view: StateView | null
  /** The same objects as `view.objects`, by id; the values are the very same references. */
  objectsById: Readonly<Record<ObjectId, ObjectView>>
  /** The log position of `view`, which is the `from_seq` the next batch must have; null while there is no view. */
  seq: number | null
  /** The decision this seat must answer now, if any. */
  pending: PendingDecision | null
  /** The clock of `pending` while it has one, counted down from the moment the decision arrived. */
  clock: DecisionClock | null
  /** The most recent events (at most `LOG_LIMIT`), oldest first. */
  log: readonly LogEntry[]
  /** The most recent chat lines (at most `CHAT_LIMIT`), oldest first. */
  chat: readonly ChatLine[]
  /** Lobby, playing or over. */
  phase: Phase
  /** The lobby roster the server pushed last; null before the first one, and again once the game has started. */
  lobby: LobbyRoster | null
  /** How the game ended, once it has. */
  result: GameResult | null
  /** The latest thing worth telling the player (a refused action, a lost seat), or null; clear it with `dismissNotice`. */
  notice: string | null
  /**
   * Whether the server passes priority for this seat when it has nothing to do (the "Skip turns when I can't act"
   * switch). The server's word: set by every Welcome and by a switch that went out; on until told otherwise.
   */
  autoPass: boolean
}

/** The store type: a vanilla zustand store; in React read it with `useStore(store, selector)` from `zustand`. */
export type PlayStore = StoreApi<PlayState>

/** The state of a session that has not connected yet. */
export function initialPlayState(): PlayState {
  return {
    status: 'idle',
    attempt: 0,
    seat: null,
    seatCount: null,
    view: null,
    objectsById: {},
    seq: null,
    pending: null,
    clock: null,
    log: [],
    chat: [],
    phase: 'lobby',
    lobby: null,
    result: null,
    notice: null,
    autoPass: true,
  }
}

/** A fresh store holding `initialPlayState()`. */
export function createPlayStore(): PlayStore {
  return createStore<PlayState>(() => initialPlayState())
}

/** `log` with a batch's events added at the end, keeping only the newest `LOG_LIMIT`. */
export function appendLog(
  log: readonly LogEntry[],
  fromSeq: number,
  events: readonly EventView[],
): readonly LogEntry[] {
  if (events.length === 0) return log
  const added = events.map((event, index): LogEntry => ({ seq: fromSeq + index, event }))
  return [...log, ...added].slice(-LOG_LIMIT)
}

/** `log` without the entries at or after `seq`, which a snapshot at `seq` makes untrue. */
export function dropLogFrom(log: readonly LogEntry[], seq: number): readonly LogEntry[] {
  return log.some((entry) => entry.seq >= seq) ? log.filter((entry) => entry.seq < seq) : log
}

/** `chat` with one line added, keeping only the newest `CHAT_LIMIT`. */
export function appendChat(chat: readonly ChatLine[], line: ChatLine): readonly ChatLine[] {
  return [...chat, line].slice(-CHAT_LIMIT)
}
