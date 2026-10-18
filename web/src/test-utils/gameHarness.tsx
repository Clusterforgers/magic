import { act } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { AuthProvider } from '@/features/auth/AuthProvider'
import type { AuthStore } from '@/features/auth/store'
import { GamePage } from '@/features/game/GamePage'
import type { SeatLink } from '@/lib/seatLink'
import type { Api } from '@/net/api'
import { FakeNetwork, FakeTimers } from './fakes'
import type { FakeSocket } from './fakes'
import { startServer, tokenOf } from './fakeServer'
import type { FakeServer, ServerScript } from './fakeServer'
import { renderInRouter } from './router'
import { welcome } from './socketSetup'

/** Options for `renderGame`. */
export interface GameOptions {
  /** The seat link from the address; the token of seat 0 by default. `null` is an address without one. */
  link?: SeatLink | null
  /** The username of the signed-in account, which joins through the session when `link` is `null`. */
  account?: string | null
  /** The sign-in state of the page; without one the page is a guest's on a server with no accounts. */
  auth?: AuthStore
  /** How the fake REST server answers. */
  script?: ServerScript
  strict?: boolean
}

/** A game page on fake sockets, a manual clock and an in-memory REST server, with the server's part in helpers. */
export interface GameHarness {
  net: FakeNetwork
  timers: FakeTimers
  server: FakeServer
  api: Api
  user: ReturnType<typeof userEvent.setup>
  container: HTMLElement
  /** The newest socket the page opened. */
  socket(): FakeSocket
  /** Opens the newest socket and answers its Join with a Welcome for `seat` of `seats`. */
  join(seat?: number, seats?: number): void
  /** Sends the newest socket these messages, in order. */
  deliver(...messages: unknown[]): void
  /** The newest socket drops without a close code, as a refused handshake looks to a browser. */
  fail(): void
  /** The server closes the newest socket with this code. */
  close(code: number, reason?: string): void
  /** Moves the manual clock, firing retries and polls that come due. */
  advance(ms: number): Promise<void>
  /** Lets pending promises (REST answers) settle. */
  settle(): Promise<void>
  unmount(): void
}

/** The game id of every harness page. */
export const GAME_ID = '8f2c1a52-31de-4c77-9a35-0d1d3f6a7b10'

/** Renders the game page and lets its first microtask run, which is when it connects. */
export async function renderGame(options: GameOptions = {}): Promise<GameHarness> {
  const timers = new FakeTimers()
  const net = new FakeNetwork(() => timers.now())
  const server = startServer(options.script)
  const link = options.link === undefined ? { seat: 0, token: tokenOf(0) } : options.link
  const deps = { createSocket: net.factory, timers, random: () => 0, origin: 'http://localhost:5173' }
  const page = (
    <GamePage gameId={GAME_ID} link={link} account={options.account ?? null} api={server.api} deps={deps} />
  )
  const view = await renderInRouter(
    options.auth === undefined ? page : <AuthProvider store={options.auth}>{page}</AuthProvider>,
    { strict: options.strict },
  )
  const settle = async (): Promise<void> => {
    await act(async () => {
      await Promise.resolve()
    })
  }
  await settle()
  return {
    net,
    timers,
    server,
    api: server.api,
    user: userEvent.setup(),
    container: view.container,
    socket: () => net.last,
    join(seat = 0, seats = 2) {
      act(() => {
        net.last.open()
        net.last.receive({ ...welcome, player: seat, seats })
      })
    },
    deliver(...messages) {
      act(() => {
        for (const message of messages) net.last.receive(message)
      })
    },
    fail() {
      act(() => net.last.fail())
    },
    close(code, reason) {
      act(() => net.last.serverClose(code, reason))
    },
    async advance(ms) {
      await act(async () => {
        timers.advance(ms)
        await Promise.resolve()
      })
    },
    settle,
    unmount: view.unmount,
  }
}
