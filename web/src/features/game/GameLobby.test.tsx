// Goal: the lobby shows the seats by name with who is connected (kept fresh by polling, honest when the server cannot be asked), lets this seat say Ready once, lists the other seats' links when this tab made the game, and works when the browser stores nothing.
import { screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { saveInvites } from '@/lib/invites'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { tokenOf } from '@/test-utils/fakeServer'
import { GAME_ID, renderGame } from '@/test-utils/gameHarness'
import { lateJoin } from '@/test-utils/transcripts'
import { LOBBY_POLL_MS } from './useGameInfo'

afterEach(() => {
  vi.restoreAllMocks()
  window.sessionStorage.clear()
})

const info = (connected: number[], seats = 2) => ({
  started: false,
  seats: ['Ana', 'Bo', 'Cy', 'Di'].slice(0, seats).map((name, seat) => ({ seat, name })),
  connected,
  result: null,
})

const rows = () => within(screen.getByRole('list', { name: 'Seats' })).getAllByRole('listitem')

describe('who is at the table', () => {
  it('lists each seat by name with who is here, and marks this seat as yours', async () => {
    const h = await renderGame({ script: { game: () => jsonResponse(200, info([1])) } })
    h.join()
    await h.settle()
    expect(rows()).toHaveLength(2)
    expect(rows()[0]).toHaveTextContent('Seat 1Anayou')
    expect(rows()[0]).toHaveTextContent('Here')
    expect(rows()[1]).toHaveTextContent('Seat 2Bo')
    expect(rows()[1]).toHaveTextContent('Here')
    expect(screen.getByText('2 of 2 here')).toBeInTheDocument()
  })

  it('shows a seat that has not connected as not here yet', async () => {
    const h = await renderGame({ script: { game: () => jsonResponse(200, info([])) } })
    h.join()
    await h.settle()
    expect(rows()[1]).toHaveTextContent('Not here yet')
    expect(screen.getByText('1 of 2 here')).toBeInTheDocument()
  })

  it('takes this seat as here from its own connection, whatever REST last said', async () => {
    const h = await renderGame({ script: { game: () => jsonResponse(200, info([1])) } })
    h.join()
    await h.settle()
    h.fail()
    expect(rows()[0]).toHaveTextContent('Not here yet')
  })

  it('shows every seat of the table before the names arrive', async () => {
    let answer: (response: Response) => void = () => undefined
    const slow = new Promise<Response>((resolve) => {
      answer = resolve
    })
    const h = await renderGame({ script: { game: () => slow } })
    h.join(0, 4)
    expect(rows().map((row) => row.textContent)).toEqual([
      expect.stringContaining('Seat 1Seat 1'),
      expect.stringContaining('Seat 2Seat 2'),
      expect.stringContaining('Seat 3Seat 3'),
      expect.stringContaining('Seat 4Seat 4'),
    ])
    answer(jsonResponse(200, info([2], 4)))
    await h.settle()
    expect(rows()[2]).toHaveTextContent('Seat 3Cy')
    expect(rows()[2]).toHaveTextContent('Here')
  })

  it('asks again every few seconds and shows a player who arrives', async () => {
    let connected: number[] = []
    const h = await renderGame({ script: { game: () => jsonResponse(200, info(connected)) } })
    h.join()
    await h.settle()
    expect(rows()[1]).toHaveTextContent('Not here yet')
    expect(h.server.count('/api/games/' + GAME_ID)).toBe(1)
    connected = [1]
    await h.advance(LOBBY_POLL_MS - 1)
    expect(h.server.count('/api/games/' + GAME_ID)).toBe(1)
    await h.advance(1)
    await h.settle()
    expect(h.server.count('/api/games/' + GAME_ID)).toBe(2)
    expect(rows()[1]).toHaveTextContent('Here')
  })

  it('stops asking once the game has started', async () => {
    const h = await renderGame({ script: { game: () => jsonResponse(200, info([1])) } })
    h.join()
    await h.settle()
    h.deliver(lateJoin.raw[1])
    await h.settle()
    const asked = h.server.count('/api/games/' + GAME_ID)
    await h.advance(10 * LOBBY_POLL_MS)
    expect(h.server.count('/api/games/' + GAME_ID)).toBe(asked)
  })

  it('says it could not load the names when the server cannot be asked, and recovers', async () => {
    let up = false
    const h = await renderGame({
      script: { game: () => (up ? jsonResponse(200, info([1])) : Promise.reject(new TypeError('offline'))) },
    })
    h.join()
    await h.settle()
    expect(screen.getByText(/could not load the names/i)).toBeInTheDocument()
    expect(rows()[1]).toHaveTextContent('Seat 2Seat 2')
    up = true
    await h.advance(LOBBY_POLL_MS)
    await h.settle()
    expect(screen.queryByText(/could not load the names/i)).toBeNull()
    expect(rows()[1]).toHaveTextContent('Seat 2Bo')
  })

  it('keeps the last names it had when a later look fails', async () => {
    let up = true
    const h = await renderGame({
      script: { game: () => (up ? jsonResponse(200, info([1])) : Promise.reject(new TypeError('offline'))) },
    })
    h.join()
    await h.settle()
    up = false
    await h.advance(LOBBY_POLL_MS)
    await h.settle()
    expect(rows()[1]).toHaveTextContent('Seat 2Bo')
    expect(screen.queryByText(/could not load the names/i)).toBeNull()
  })
})

describe('saying Ready', () => {
  it('sends Ready once, shows that it did, and cannot be pressed again', async () => {
    const h = await renderGame()
    h.join()
    const button = screen.getByRole('button', { name: 'Ready' })
    await h.user.click(button)
    expect(h.socket().sentTypes).toEqual(['Join', 'Ready'])
    expect(screen.getByRole('button', { name: 'You are ready' })).toBeDisabled()
    expect(screen.getByText(/waiting for the other players/i)).toBeInTheDocument()
    await h.user.click(screen.getByRole('button', { name: 'You are ready' }))
    expect(h.socket().sentTypes).toEqual(['Join', 'Ready'])
  })

  it('is not offered while the connection is down', async () => {
    const h = await renderGame()
    h.join()
    h.fail()
    expect(screen.getByRole('button', { name: 'Ready' })).toBeDisabled()
    await h.advance(250)
    h.join()
    expect(screen.getByRole('button', { name: 'Ready' })).toBeEnabled()
  })

  it('remembers that this seat is ready across a reconnect', async () => {
    const h = await renderGame()
    h.join()
    await h.user.click(screen.getByRole('button', { name: 'Ready' }))
    h.fail()
    await h.advance(250)
    h.join()
    expect(screen.getByRole('button', { name: 'You are ready' })).toBeDisabled()
  })

  it('says plainly that the server does not tell who else is ready', async () => {
    const h = await renderGame()
    h.join()
    expect(screen.getByText(/only says who is connected, not who has pressed Ready/i)).toBeInTheDocument()
  })
})

describe('inviting the other players', () => {
  const stash = () =>
    saveInvites({
      game: GAME_ID,
      seats: [
        { seat: 0, name: 'Ana', token: tokenOf(0) },
        { seat: 1, name: 'Bo', token: tokenOf(1) },
        { seat: 2, name: 'Cy', token: tokenOf(2) },
      ],
    })

  it('lists a link for every other seat, in the documented form, and none for this seat', async () => {
    stash()
    const h = await renderGame()
    h.join(0, 3)
    const list = screen.getByRole('list', { name: 'Seat links' })
    const links = within(list).getAllByRole('link')
    expect(links.map((link) => link.getAttribute('href'))).toEqual([
      `/game/${GAME_ID}#seat=1&token=${tokenOf(1)}`,
      `/game/${GAME_ID}#seat=2&token=${tokenOf(2)}`,
    ])
    for (const link of links) expect(link).toHaveAttribute('data-share-link')
    expect(within(list).getByText('Seat 2: Bo')).toBeInTheDocument()
    expect(within(list).queryByText(/Ana/)).toBeNull()
  })

  it('copies one seat link', async () => {
    stash()
    const h = await renderGame()
    h.join(0, 3)
    await h.user.click(screen.getByRole('button', { name: 'Copy link for seat 3 (Cy)' }))
    expect(await navigator.clipboard.readText()).toBe(
      `${window.location.origin}/game/${GAME_ID}#seat=2&token=${tokenOf(2)}`,
    )
  })

  it('has no section for it when this tab did not make the game', async () => {
    const h = await renderGame()
    h.join()
    expect(screen.queryByText('Invite the other players')).toBeNull()
  })

  it('has none for a game that is not this one', async () => {
    saveInvites({ game: 'another-game', seats: [{ seat: 1, name: 'Bo', token: tokenOf(1) }] })
    const h = await renderGame()
    h.join()
    expect(screen.queryByText('Invite the other players')).toBeNull()
  })

  it('works, without the section, when the browser will not give out storage', async () => {
    vi.spyOn(window, 'sessionStorage', 'get').mockImplementation(() => {
      throw new DOMException('blocked', 'SecurityError')
    })
    const h = await renderGame()
    h.join()
    expect(screen.getByRole('heading', { name: 'Waiting for players' })).toBeInTheDocument()
    expect(screen.queryByText('Invite the other players')).toBeNull()
    expect(h.socket().sent).toMatchObject([{ type: 'Join', name: 'Seat 1' }])
  })
})
