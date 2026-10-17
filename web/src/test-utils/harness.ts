import { isRecord } from '@/net/check'
import { createSession } from '@/play/session'
import type { Session, SessionDeps } from '@/play/session'
import type { PlayState } from '@/play/store'
import type { Answer, ServerMessage } from '@/protocol/generated'
import { FakeNetwork, FakeTimers } from './fakes'
import { GAME, TOKEN, welcome } from './socketSetup'

/** A session on fake sockets and a manual clock, with helpers to play the server's part. */
export interface Harness {
  session: Session
  net: FakeNetwork
  timers: FakeTimers
  state(): PlayState
  /** Connects and completes the handshake as `seat` of `seats`. */
  join(seat?: number, seats?: number): void
  /** Opens the newest socket without answering its Join, as after a reconnect. */
  open(): void
  /** Sends the newest socket these messages, in order. */
  deliver(...messages: unknown[]): void
  /** Everything written to the newest socket, decoded. */
  sent(): unknown[]
  /** The messages of one type written to the newest socket. */
  sentOf(type: string): unknown[]
  /** The `type` of everything written to the newest socket, in order. */
  sentTypes(): string[]
  /** The decision id of every Answer written to the newest socket, in order. */
  answeredIds(): number[]
}

/** A `Decision` message asking seat 0 for priority, with the given id. */
export function priorityDecision(id: number): ServerMessage {
  const options = { playable_lands: [], castable: [], mana_abilities: [], can_pass: true }
  return { type: 'Decision', id, decision: { player: 0, kind: { Priority: options } } }
}

/** The answer that passes priority. */
export const pass: Answer = { Priority: 'Pass' }

/** Builds a session with no jitter in its retries; `overrides` replace any dependency. */
export function startSession(overrides: Partial<SessionDeps> = {}): Harness {
  const timers = new FakeTimers()
  const net = new FakeNetwork(() => timers.now())
  const session = createSession({
    game: GAME,
    token: TOKEN,
    name: 'Ana',
    origin: 'http://localhost:5173',
    createSocket: net.factory,
    timers,
    random: () => 0,
    ...overrides,
  })
  const sent = (): unknown[] => net.last.sent
  return {
    session,
    net,
    timers,
    state: () => session.store.getState(),
    join(seat = 0, seats = 2) {
      session.connect()
      net.last.open()
      net.last.receive({ ...welcome, player: seat, seats })
    },
    open: () => net.last.open(),
    deliver(...messages) {
      for (const message of messages) net.last.receive(message)
    },
    sent,
    sentOf: (type) => sent().filter((message) => isRecord(message) && message.type === type),
    sentTypes: () => net.last.sentTypes,
    answeredIds: () =>
      sent().flatMap((message) =>
        isRecord(message) && message.type === 'Answer' && typeof message.decision === 'number'
          ? [message.decision]
          : [],
      ),
  }
}
