// Goal: the page moves from lobby to the table when the game starts however the state arrives, keeps the table under a reconnect message, and ends on a result screen that says who won, a draw or a concession, with the way to a new game and no connection notice.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { GameResult, LossReason, StateView } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { renderGame } from '@/test-utils/gameHarness'
import { lateJoin, lobbyBatchAt, lobbyStart } from '@/test-utils/transcripts'

const names = {
  started: true,
  seats: [
    { seat: 0, name: 'Ana' },
    { seat: 1, name: 'Bo' },
  ],
  connected: [0, 1],
  result: null,
}

const table = () => screen.getByRole('region', { name: 'Game table' })

/** The transcript's final view, with seat `loser` having lost for `reason` and the game decided by `result`. */
function endedView(loser: number, reason: LossReason): StateView {
  const view = clone(lateJoin.expected)
  for (const player of view.players) player.lost = player.seat === loser ? reason : null
  return view
}

async function finishedGame(seat: number, loser: number, reason: LossReason, result: GameResult) {
  const h = await renderGame({
    link: { seat, token: 'tok' },
    script: { game: () => jsonResponse(200, names) },
  })
  h.join(seat)
  h.deliver({ type: 'Snapshot', seq: 0, state: endedView(loser, reason) })
  h.deliver({ type: 'GameOver', result })
  await h.settle()
  return h
}

describe('playing', () => {
  it.each([
    ['a snapshot, as a seat that joins a running game gets', lateJoin.raw.slice(1, 2)],
    [
      'the first batch, as a seat that was in the lobby gets',
      lobbyStart.raw.slice(lobbyBatchAt, lobbyBatchAt + 1),
    ],
  ])('shows the table with a panel for each seat once %s arrives', (_how, messages) => {
    return renderGame().then((h) => {
      h.join()
      expect(screen.getByRole('heading', { name: 'Waiting for players' })).toBeInTheDocument()
      h.deliver(...messages)
      expect(screen.queryByRole('heading', { name: 'Waiting for players' })).toBeNull()
      expect(table().querySelectorAll('[data-seat]')).toHaveLength(2)
      expect(document.title).toBe('Game · Magic')
    })
  })

  it('keeps following the game while it plays, without touching the screen', async () => {
    const h = await renderGame()
    h.join()
    h.deliver(...lateJoin.raw.slice(1, -1))
    expect(table().querySelectorAll('[data-seat]')).toHaveLength(2)
    expect(screen.queryByRole('heading', { name: 'Game over' })).toBeNull()
  })

  it('keeps the table on screen under the message when the line drops', async () => {
    const h = await renderGame()
    h.join()
    h.deliver(lateJoin.raw[1])
    h.fail()
    expect(screen.getByRole('status')).toHaveTextContent('Reconnecting… (attempt 1)')
    expect(table()).toBeInTheDocument()
    await h.advance(250)
    h.join()
    h.deliver(lateJoin.raw[1])
    expect(screen.queryByText(/Reconnecting/)).toBeNull()
    expect(table()).toBeInTheDocument()
  })

  it('floats the message over the table instead of pushing it down, since the table is exactly the window', async () => {
    const h = await renderGame()
    h.join()
    h.deliver(lateJoin.raw[1])
    h.fail()
    const notices = must(document.querySelector<HTMLElement>('[data-slot="table-notices"]'))
    expect(notices).toHaveClass('fixed', 'inset-x-0', 'top-11')
    expect(notices).toContainElement(screen.getByRole('status'))
    expect(table().contains(notices)).toBe(false)
    expect(screen.getByRole('status').closest('[data-slot="page"]')).toBeNull()
  })

  it('keeps the message in the page, as before, on the lobby', async () => {
    const h = await renderGame()
    h.join()
    h.fail()
    expect(document.querySelector('[data-slot="table-notices"]')).toBeNull()
    expect(screen.getByRole('status').closest('[data-slot="page"]')).not.toBeNull()
  })

  it('says it cannot reach the game above the table after five failures', async () => {
    const h = await renderGame()
    h.join()
    h.deliver(lateJoin.raw[1])
    for (let attempt = 1; attempt <= 5; attempt += 1) {
      h.fail()
      if (attempt < 5) await h.advance(15_000)
    }
    expect(screen.getByRole('alert')).toHaveTextContent('Cannot reach the game')
    expect(table()).toBeInTheDocument()
    expect(screen.queryByRole('heading', { level: 1, name: 'Cannot reach the game' })).toBeNull()
    expect(screen.getByRole('heading', { level: 2, name: 'Cannot reach the game' })).toBeInTheDocument()
  })
})

describe('game over', () => {
  it('tells the winner, with how the other seat lost, and offers a new game', async () => {
    const h = await renderGame({ script: { game: () => jsonResponse(200, names) } })
    h.join()
    h.deliver(...lateJoin.raw.slice(1))
    await h.settle()
    expect(screen.getByRole('heading', { name: 'You won' })).toHaveFocus()
    expect(screen.getByText('Bo ran out of life.')).toBeInTheDocument()
    const results = within(screen.getByRole('list', { name: 'How each seat finished' })).getAllByRole(
      'listitem',
    )
    expect(results[0]).toHaveTextContent('Seat 1Anayou')
    expect(results[0]).toHaveTextContent('Winner')
    expect(results[1]).toHaveTextContent('Seat 2Bo')
    expect(results[1]).toHaveTextContent('Lost: ran out of life')
    expect(results[1]).toHaveTextContent('life 0')
    expect(screen.getByRole('link', { name: 'Create a new game' })).toHaveAttribute('href', '/')
    expect(document.title).toBe('Game over · Magic')
  })

  it('keeps the result when the server closes the socket afterwards, with no connection notice', async () => {
    const h = await renderGame()
    h.join()
    h.deliver(...lateJoin.raw.slice(1))
    h.close(1000)
    expect(screen.getByRole('heading', { name: 'You won' })).toBeInTheDocument()
    expect(screen.queryByRole('alert')).toBeNull()
    expect(screen.queryByRole('status')).toBeNull()
    await h.advance(60_000)
    expect(h.net.sockets).toHaveLength(1)
  })

  it('reports a concession by the other seat as a win that says who gave up', async () => {
    await finishedGame(0, 1, 'Conceded', { Winner: 0 })
    expect(screen.getByRole('heading', { name: 'You won' })).toBeInTheDocument()
    expect(screen.getByText('Bo conceded.')).toBeInTheDocument()
  })

  it('reports this seat’s own concession as a loss', async () => {
    await finishedGame(0, 0, 'Conceded', { Winner: 1 })
    expect(screen.getByRole('heading', { name: 'You lost' })).toBeInTheDocument()
    expect(screen.getByText('Bo won. You conceded.')).toBeInTheDocument()
    const results = within(screen.getByRole('list', { name: 'How each seat finished' })).getAllByRole(
      'listitem',
    )
    expect(results[0]).toHaveTextContent('Lost: conceded')
    expect(results[1]).toHaveTextContent('Winner')
  })

  it('reports a loss on the board as a loss', async () => {
    await finishedGame(0, 0, 'EmptyLibrary', { Winner: 1 })
    expect(screen.getByRole('heading', { name: 'You lost' })).toBeInTheDocument()
    expect(screen.getByText('Bo won. You could not draw from an empty library.')).toBeInTheDocument()
  })

  it('reports a draw', async () => {
    const h = await renderGame()
    h.join()
    h.deliver({ type: 'GameOver', result: 'Draw' })
    expect(screen.getByRole('heading', { name: 'The game is a draw' })).toBeInTheDocument()
    expect(screen.getByText('Nobody won.')).toBeInTheDocument()
    expect(screen.queryByRole('list', { name: 'How each seat finished' })).toBeNull()
    expect(screen.getByRole('link', { name: 'Create a new game' })).toBeInTheDocument()
  })

  it('names seats “Seat N” when the server could not be asked for names', async () => {
    const h = await renderGame()
    h.join()
    h.deliver(...lateJoin.raw.slice(1))
    await h.settle()
    expect(screen.getByText('Seat 2 ran out of life.')).toBeInTheDocument()
    const results = within(screen.getByRole('list', { name: 'How each seat finished' })).getAllByRole(
      'listitem',
    )
    expect(must(results[1])).toHaveTextContent('Seat 2Seat 2')
  })

  it('shows the result even if this page never got a view of the game', async () => {
    const h = await renderGame()
    h.join(1)
    h.deliver({ type: 'GameOver', result: { Winner: 0 } })
    expect(screen.getByRole('heading', { name: 'Seat 1 won' })).toBeInTheDocument()
  })
})
