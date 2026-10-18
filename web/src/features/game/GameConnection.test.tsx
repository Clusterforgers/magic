// Goal: the game page shows the right screen for every state of the connection (no seat link, connecting, reconnecting with its attempt number, cannot reach after five failures with a retry that starts over, a refused or replaced seat, the server's notice) and never leaves two sockets open.
import { screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { saveInvites } from '@/lib/invites'
import { PROTOCOL_VERSION } from '@/protocol/generated'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { tokenOf } from '@/test-utils/fakeServer'
import { GAME_ID, renderGame } from '@/test-utils/gameHarness'
import type { GameHarness } from '@/test-utils/gameHarness'
import { UNREACHABLE_AFTER } from './connection'

afterEach(() => {
  window.sessionStorage.clear()
})

const status = () => screen.getByRole('status')
const alert = () => screen.getByRole('alert')

/** Fails the newest socket `count` times, letting each retry open in between, and stops right after the last failure. */
async function failTimes(h: GameHarness, count: number): Promise<void> {
  for (let attempt = 1; attempt <= count; attempt += 1) {
    h.fail()
    if (attempt < count) await h.advance(15_000)
  }
}

describe('an address without a seat link', () => {
  it('asks for the seat link and connects to nothing', async () => {
    const h = await renderGame({ link: null })
    expect(screen.getByRole('heading', { name: 'Open your seat link' })).toBeInTheDocument()
    expect(screen.getByText(/ask the host to send it again/i)).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Create a new game' })).toHaveAttribute('href', '/')
    expect(h.net.sockets).toHaveLength(0)
    expect(h.server.calls).toHaveLength(0)
    expect(document.title).toBe('Open your seat link · Magic')
  })
})

describe('connecting', () => {
  it('says so, opens one socket at the seat address and joins once with the seat name', async () => {
    const h = await renderGame()
    expect(screen.getByRole('heading', { name: 'Connecting to the game…' })).toBeInTheDocument()
    expect(status()).toHaveTextContent('Connecting to the game…')
    expect(h.net.sockets).toHaveLength(1)
    expect(h.socket().url).toBe(`ws://localhost:5173/ws/${GAME_ID}/${tokenOf(0)}`)
    h.socket().open()
    expect(h.socket().sent).toEqual([
      { type: 'Join', protocol_version: PROTOCOL_VERSION, game: GAME_ID, name: 'Seat 1' },
    ])
    expect(status()).toHaveTextContent('Connecting to the game…')
  })

  it('joins under the name the host gave the seat when this tab made the game', async () => {
    saveInvites({ game: GAME_ID, seats: [{ seat: 0, name: 'Ana', token: tokenOf(0) }] })
    const h = await renderGame()
    h.socket().open()
    expect(h.socket().sent).toMatchObject([{ type: 'Join', name: 'Ana' }])
  })

  it('moves on to the lobby once the server says Welcome', async () => {
    const h = await renderGame()
    h.join()
    expect(screen.getByRole('heading', { name: 'Waiting for players' })).toBeInTheDocument()
    expect(screen.queryByText('Connecting to the game…')).toBeNull()
  })
})

describe('reconnecting', () => {
  it.each([1, 2, 3, 4])('shows attempt %i of the retries by number', async (attempt) => {
    const h = await renderGame()
    await failTimes(h, attempt)
    expect(status()).toHaveTextContent(`Connection lost. Reconnecting… (attempt ${attempt})`)
    expect(screen.queryByText('Cannot reach the game')).toBeNull()
  })

  it('keeps the lobby on screen under the message when a seat that had joined loses its line', async () => {
    const h = await renderGame()
    h.join()
    h.fail()
    expect(status()).toHaveTextContent('Reconnecting… (attempt 1)')
    expect(screen.getByRole('heading', { name: 'Waiting for players' })).toBeInTheDocument()
    await h.advance(250)
    h.join()
    expect(screen.queryByText(/Reconnecting/)).toBeNull()
    expect(screen.getByRole('heading', { name: 'Waiting for players' })).toBeInTheDocument()
  })
})

describe('cannot reach the game', () => {
  it(`appears only after ${UNREACHABLE_AFTER} failed attempts, with a retry button`, async () => {
    const h = await renderGame()
    await failTimes(h, UNREACHABLE_AFTER - 1)
    expect(screen.queryByText('Cannot reach the game')).toBeNull()
    await h.advance(15_000)
    h.fail()
    expect(alert()).toHaveTextContent('Cannot reach the game')
    expect(alert()).toHaveTextContent(`tried ${UNREACHABLE_AFTER} times`)
    expect(screen.getByRole('heading', { name: 'Cannot reach the game' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Try again now' })).toBeEnabled()
    expect(screen.getByRole('link', { name: 'Create a new game' })).toHaveAttribute('href', '/')
  })

  it('names every likely cause, since a wrong token looks like a lost connection', async () => {
    const h = await renderGame()
    await failTimes(h, UNREACHABLE_AFTER)
    expect(alert()).toHaveTextContent(/server may be down/i)
    expect(alert()).toHaveTextContent(/network may be offline/i)
    expect(alert()).toHaveTextContent(/seat link may be wrong or out of date/i)
  })

  it('adds that the game does not exist when the server says so', async () => {
    const h = await renderGame()
    await failTimes(h, UNREACHABLE_AFTER)
    expect(alert()).toHaveTextContent(/does not exist, so the link is probably out of date/i)
  })

  it('leaves that out when the server knows the game', async () => {
    const info = { started: false, seats: [{ seat: 0, name: 'Ana' }], connected: [], result: null }
    const h = await renderGame({ script: { game: () => jsonResponse(200, info) } })
    await failTimes(h, UNREACHABLE_AFTER)
    expect(alert()).not.toHaveTextContent(/does not exist/i)
  })

  it('keeps trying in the background and goes on by itself when a later try works', async () => {
    const h = await renderGame()
    await failTimes(h, UNREACHABLE_AFTER)
    await h.advance(15_000)
    expect(h.net.sockets).toHaveLength(UNREACHABLE_AFTER + 1)
    h.join()
    expect(screen.queryByText('Cannot reach the game')).toBeNull()
    expect(screen.getByRole('heading', { name: 'Waiting for players' })).toBeInTheDocument()
  })

  it('starts over at once when Try again now is pressed, counting attempts from 1', async () => {
    const h = await renderGame()
    await failTimes(h, UNREACHABLE_AFTER)
    const before = h.net.sockets.length
    await h.user.click(screen.getByRole('button', { name: 'Try again now' }))
    expect(h.net.sockets).toHaveLength(before + 1)
    expect(screen.getByRole('heading', { name: 'Connecting to the game…' })).toBeInTheDocument()
    expect(screen.queryByText('Cannot reach the game')).toBeNull()
    h.fail()
    expect(status()).toHaveTextContent('(attempt 1)')
  })
})

describe('a connection that ended for good', () => {
  it('explains a refusal in the server’s words and connects again on request', async () => {
    const h = await renderGame()
    h.join()
    h.close(1008, 'too many messages')
    expect(screen.getByRole('heading', { name: 'The connection ended' })).toBeInTheDocument()
    expect(alert()).toHaveTextContent(/policy violation \(too many messages\)/)
    expect(screen.queryByRole('heading', { name: 'Waiting for players' })).toBeNull()
    await h.user.click(screen.getByRole('button', { name: 'Connect again' }))
    expect(h.net.sockets).toHaveLength(2)
    expect(
      screen.getByRole('heading', { name: 'Connecting to the game…' }).closest('[role=status]'),
    ).not.toBeNull()
    h.join()
    expect(screen.getByRole('heading', { name: 'Waiting for players' })).toBeInTheDocument()
  })

  it('says the game is gone when the server does not know it', async () => {
    const h = await renderGame()
    h.join()
    h.deliver({ type: 'Error', code: 'UnknownGame', message: 'no such game' })
    expect(alert()).toHaveTextContent('This game does not exist any more.')
  })

  it('says the seat is open somewhere else, and takes it back on request', async () => {
    const h = await renderGame()
    h.join()
    h.close(4000)
    expect(screen.getByRole('heading', { name: 'This seat is open somewhere else' })).toBeInTheDocument()
    expect(alert()).toHaveTextContent(/opened in another window or tab/i)
    await h.user.click(screen.getByRole('button', { name: 'Use this window instead' }))
    expect(h.net.sockets).toHaveLength(2)
    h.join()
    expect(screen.getByRole('heading', { name: 'Waiting for players' })).toBeInTheDocument()
  })
})

describe('the server’s notice', () => {
  it('shows what the server refused, and goes away when dismissed', async () => {
    const h = await renderGame()
    h.join()
    h.deliver({ type: 'Error', code: 'BadMessage', message: 'not now' })
    expect(alert()).toHaveTextContent('The server did not accept that: not now')
    await h.user.click(screen.getByRole('button', { name: 'Dismiss' }))
    expect(screen.queryByRole('alert')).toBeNull()
  })

  it('is shown once, not again beside the explanation of a stopped connection', async () => {
    const h = await renderGame()
    h.join()
    h.close(1002)
    expect(screen.getAllByRole('alert')).toHaveLength(1)
  })
})

describe('sockets', () => {
  it('opens exactly one socket even when React mounts everything twice', async () => {
    const h = await renderGame({ strict: true })
    expect(h.net.sockets).toHaveLength(1)
    expect(h.socket().closedByClient).toBeNull()
    h.join()
    expect(screen.getByRole('heading', { name: 'Waiting for players' })).toBeInTheDocument()
    expect(h.socket().sentTypes).toEqual(['Join'])
  })

  it('closes its socket when the page goes away', async () => {
    const h = await renderGame()
    h.join()
    h.unmount()
    expect(h.socket().closedByClient).toEqual({ code: 1000, reason: 'client closing' })
    await h.advance(60_000)
    expect(h.net.sockets).toHaveLength(1)
  })

  it('stops retrying when the page goes away in the middle of a retry', async () => {
    const h = await renderGame()
    h.fail()
    h.unmount()
    await h.advance(60_000)
    expect(h.net.sockets).toHaveLength(1)
  })
})
