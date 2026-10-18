// Goal: before the server's roster arrives (and while the socket is down) the lobby shows the seats by name with who is connected and who is ready from the REST lookup, kept fresh by polling and honest when the server cannot be asked; this seat's Ready can be said and taken back; it lists the other seats' links when this tab made the game, and works when the browser stores nothing.
import { screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { saveInvites } from '@/lib/invites'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { tokenOf } from '@/test-utils/fakeServer'
import { GAME_ID, renderGame } from '@/test-utils/gameHarness'
import { lobbyMessage } from '@/test-utils/lobbyFrames'
import { lateJoin } from '@/test-utils/transcripts'
import { LOBBY_POLL_MS } from './useGameInfo'

afterEach(() => {
  vi.restoreAllMocks()
  window.sessionStorage.clear()
})

const info = (connected: number[], seats = 2, ready: number[] = []) => ({
  started: false,
  seats: ['Ana', 'Bo', 'Cy', 'Di'].slice(0, seats).map((name, seat) => ({ seat, name })),
  connected,
  ready,
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
    expect(rows()[0]).toHaveTextContent('Not ready yet')
    expect(rows()[1]).toHaveTextContent('Seat 2Bo')
    expect(rows()[1]).toHaveTextContent('Not ready yet')
    expect(screen.getByText('0 of 2 ready, waiting for you and Bo to press Ready')).toBeInTheDocument()
  })

  it('shows a seat that has not connected as not here', async () => {
    const h = await renderGame({ script: { game: () => jsonResponse(200, info([])) } })
    h.join()
    await h.settle()
    expect(rows()[1]).toHaveTextContent('Not here')
    expect(
      screen.getByText('0 of 2 ready, waiting for you to press Ready and Bo to join'),
    ).toBeInTheDocument()
  })

  it('takes this seat as here from its own connection, whatever REST last said', async () => {
    const h = await renderGame({ script: { game: () => jsonResponse(200, info([1])) } })
    h.join()
    await h.settle()
    h.fail()
    expect(rows()[0]).toHaveTextContent('Not here')
  })

  it('shows who is ready from the lookup, before any roster has come, away or not', async () => {
    const h = await renderGame({ script: { game: () => jsonResponse(200, info([], 3, [1, 2])) } })
    h.join(0, 3)
    await h.settle()
    expect(rows()[0]).toHaveTextContent('Not ready yet')
    expect(rows()[1]).toHaveTextContent('Ready')
    expect(rows()[1]).toHaveTextContent('(away)')
    expect(rows()[2]).toHaveTextContent('Ready')
    expect(screen.getByText('2 of 3 ready, waiting for you to press Ready')).toBeInTheDocument()
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
    expect(rows()[2]).toHaveTextContent('Not ready yet')
  })

  it('asks again every few seconds and shows a player who arrives', async () => {
    let connected: number[] = []
    const h = await renderGame({ script: { game: () => jsonResponse(200, info(connected)) } })
    h.join()
    await h.settle()
    expect(rows()[1]).toHaveTextContent('Not here')
    expect(h.server.count('/api/games/' + GAME_ID)).toBe(1)
    connected = [1]
    await h.advance(LOBBY_POLL_MS - 1)
    expect(h.server.count('/api/games/' + GAME_ID)).toBe(1)
    await h.advance(1)
    await h.settle()
    expect(h.server.count('/api/games/' + GAME_ID)).toBe(2)
    expect(rows()[1]).toHaveTextContent('Not ready yet')
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

describe('saying Ready and taking it back', () => {
  const table = [
    ['Ana', true, false],
    ['Bo', true, false],
  ] as const
  const ready = [
    ['Ana', true, true],
    ['Bo', true, false],
  ] as const

  it('sends Ready and shows it only once the server says so; the button then takes it back', async () => {
    const h = await renderGame()
    h.join()
    h.deliver(lobbyMessage(table))
    await h.user.click(screen.getByRole('button', { name: 'Ready' }))
    expect(h.socket().sentTypes).toEqual(['Join', 'Ready'])
    // The page shows what the server holds, not what it hopes: nothing changes until the roster says so.
    expect(screen.getByRole('button', { name: 'Ready' })).toBeInTheDocument()
    h.deliver(lobbyMessage(ready))
    const back = screen.getByRole('button', { name: 'Not ready' })
    expect(back).toBeEnabled()
    expect(screen.getByText(/you can take it back until the game starts/i)).toBeInTheDocument()
    await h.user.click(back)
    expect(h.socket().sentTypes).toEqual(['Join', 'Ready', 'NotReady'])
    h.deliver(lobbyMessage(table))
    expect(screen.getByRole('button', { name: 'Ready' })).toBeEnabled()
    expect(screen.getByText(/press ready when you are set to play/i)).toBeInTheDocument()
  })

  it('can be switched on and off as often as the player likes, each press one message', async () => {
    const h = await renderGame()
    h.join()
    h.deliver(lobbyMessage(table))
    for (const [name, shown] of [
      ['Ready', ready],
      ['Not ready', table],
      ['Ready', ready],
      ['Not ready', table],
    ] as const) {
      await h.user.click(screen.getByRole('button', { name }))
      h.deliver(lobbyMessage(shown))
    }
    expect(h.socket().sentTypes).toEqual(['Join', 'Ready', 'NotReady', 'Ready', 'NotReady'])
  })

  it('takes a double click for one press, since the label may have flipped between the two clicks', async () => {
    const h = await renderGame()
    h.join()
    h.deliver(lobbyMessage(table))
    await h.user.dblClick(screen.getByRole('button', { name: 'Ready' }))
    expect(h.socket().sentTypes).toEqual(['Join', 'Ready'])
    h.deliver(lobbyMessage(ready))
    await h.user.dblClick(screen.getByRole('button', { name: 'Not ready' }))
    expect(h.socket().sentTypes).toEqual(['Join', 'Ready', 'NotReady'])
  })

  it('answers the keyboard as well as the mouse', async () => {
    const h = await renderGame()
    h.join()
    h.deliver(lobbyMessage(table))
    screen.getByRole('button', { name: 'Ready' }).focus()
    await h.user.keyboard('{Enter}')
    await h.user.keyboard(' ')
    expect(h.socket().sentTypes).toEqual(['Join', 'Ready', 'Ready'])
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

  it('needs no memory of its own across a reconnect: the server says what it holds', async () => {
    const h = await renderGame()
    h.join()
    h.deliver(lobbyMessage(table))
    await h.user.click(screen.getByRole('button', { name: 'Ready' }))
    h.deliver(lobbyMessage(ready))
    h.fail()
    // The roster belonged to the connection that is gone; the button waits for the new one.
    expect(screen.getByRole('button', { name: 'Ready' })).toBeDisabled()
    await h.advance(250)
    h.join()
    h.deliver(lobbyMessage(ready))
    expect(screen.getByRole('button', { name: 'Not ready' })).toBeEnabled()
    // A reconnect sends no Ready of its own: the new socket has only said who it is.
    expect(h.socket().sentTypes).toEqual(['Join'])
  })

  it('shows another seat taking its Ready back as well as pressing it', async () => {
    const h = await renderGame()
    h.join()
    h.deliver(lobbyMessage(table))
    h.deliver(lobbyMessage([table[0], ['Bo', true, true]]))
    expect(rows()[1]).toHaveTextContent('Ready')
    expect(screen.getByText('1 of 2 ready, waiting for you to press Ready')).toBeInTheDocument()
    h.deliver(lobbyMessage(table))
    expect(rows()[1]).toHaveTextContent('Not ready yet')
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
