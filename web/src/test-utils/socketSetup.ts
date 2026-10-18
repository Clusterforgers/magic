import { GameSocket } from '@/net/socket'
import type { ConnectionStatus, GameSocketOptions, SocketEvent } from '@/net/socket'
import { PROTOCOL_VERSION } from '@/protocol/generated'
import { FakeNetwork, FakeTimers } from './fakes'

/** The game id the socket tests use. */
export const GAME = '00000000-0000-4000-8000-000000000001'
/** The seat token the socket tests use. */
export const TOKEN = 'tok-123'
/** The `Welcome` a server sends seat 0 of a two-seat game. */
export const welcome = {
  type: 'Welcome',
  protocol_version: PROTOCOL_VERSION,
  game: GAME,
  player: 0,
  seats: 2,
  auto_pass: true,
  yielding: null,
}
/** The `Join` the socket must open with. */
export const join = { type: 'Join', protocol_version: PROTOCOL_VERSION, game: GAME, name: 'Ana' }

/** A `GameSocket` on fake sockets and a manual clock, with every event it emits recorded; no jitter by default. */
export function setup(overrides: Partial<GameSocketOptions> = {}) {
  const timers = new FakeTimers()
  const net = new FakeNetwork(() => timers.now())
  const events: SocketEvent[] = []
  const socket = new GameSocket({
    game: GAME,
    token: TOKEN,
    name: 'Ana',
    origin: 'http://localhost:5173',
    onEvent: (event) => events.push(event),
    createSocket: net.factory,
    timers,
    random: () => 0,
    ...overrides,
  })
  const statuses = (): ConnectionStatus[] =>
    events.flatMap((event) => (event.type === 'status' ? [event.status] : []))
  const lastStatus = (): SocketEvent | undefined => events.findLast((event) => event.type === 'status')
  return { net, timers, events, socket, statuses, lastStatus }
}

/** What `setup` returns. */
export type Setup = ReturnType<typeof setup>

/** Connects and completes the handshake. */
export function open({ socket, net }: Setup): void {
  socket.connect()
  net.last.open()
  net.last.receive(welcome)
}

/** Milliseconds until the scheduled retry; fails the test when none is scheduled. */
export function nextWait({ timers }: Setup): number {
  const [wait] = timers.pending()
  if (wait === undefined) throw new Error('no retry is scheduled')
  return wait
}
