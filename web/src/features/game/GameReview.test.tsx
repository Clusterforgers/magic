// Goal: a finished game opens the final table and its log for a look back and closes them again: "Look at the table and log" swaps the result for the table under a strip that says the result and that the table is read only, "Show result" brings the result back with its focus and its ways on, "Back to Play" stays one click in both, and a game with no board to show offers no look back.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { GameResult } from '@/protocol/generated'
import { clone } from '@/test-utils/batches'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { renderGame } from '@/test-utils/gameHarness'
import { lateJoin } from '@/test-utils/transcripts'

const names = {
  started: true,
  seats: [
    { seat: 0, name: 'Ana' },
    { seat: 1, name: 'Bo' },
  ],
  connected: [0, 1],
  result: null,
}

/** A game that ended with `result`, as seat 0 sees it; with `board` false the page never got a view. */
async function ended(result: GameResult, board = true) {
  const h = await renderGame({ script: { game: () => jsonResponse(200, names) } })
  h.join()
  if (board) {
    const view = clone(lateJoin.expected)
    for (const player of view.players) player.lost = player.seat === 1 ? 'ZeroLife' : null
    h.deliver({ type: 'Snapshot', seq: 0, state: view })
  }
  h.deliver({ type: 'GameOver', result })
  await h.settle()
  return h
}

const lookBack = () => screen.getByRole('button', { name: 'Look at the table and log' })
const strip = () => document.querySelector<HTMLElement>('[data-slot="review-bar"]')

describe('looking back at a finished game', () => {
  it('opens the table and the log under a strip with the result, and takes no action', async () => {
    const h = await ended({ Winner: 0 })
    await h.user.click(lookBack())
    expect(screen.getByRole('region', { name: 'Game table' })).toBeInTheDocument()
    expect(screen.queryByRole('heading', { name: 'You won' })).toBeNull()
    const bar = within(strip() as HTMLElement)
    expect(bar.getByText('You won')).toBeInTheDocument()
    expect(bar.getByText('Final table, read only')).toBeInTheDocument()
    // The log is one tab of the table's side area, as in play.
    expect(screen.getAllByRole('tab', { name: /^Log/ }).length).toBeGreaterThan(0)
    // The session is closed: the table has nothing to answer and the panel says the game is over.
    expect(screen.queryByRole('group', { name: /^Priority|^Mulligan/ })).toBeNull()
    expect(document.title).toBe('Game over · Magic')
  })

  it('brings the result back with its focus and its ways on', async () => {
    const h = await ended({ Winner: 0 })
    await h.user.click(lookBack())
    await h.user.click(screen.getByRole('button', { name: 'Show result' }))
    expect(strip()).toBeNull()
    expect(screen.getByRole('heading', { name: 'You won' })).toHaveFocus()
    expect(screen.getByRole('link', { name: 'Create another game' })).toHaveAttribute('href', '/new')
    expect(screen.getByRole('link', { name: 'Back to Play' })).toHaveAttribute('href', '/')
  })

  it('keeps the way out one click away in both views', async () => {
    const h = await ended({ Winner: 0 })
    expect(screen.getByRole('link', { name: 'Back to Play' })).toHaveAttribute('href', '/')
    await h.user.click(lookBack())
    expect(within(strip() as HTMLElement).getByRole('link', { name: 'Back to Play' })).toHaveAttribute(
      'href',
      '/',
    )
  })

  it('can go back and forth any number of times', async () => {
    const h = await ended({ Winner: 0 })
    for (let round = 0; round < 3; round += 1) {
      await h.user.click(lookBack())
      expect(strip()).not.toBeNull()
      await h.user.click(screen.getByRole('button', { name: 'Show result' }))
      expect(strip()).toBeNull()
    }
  })

  it.each<[string, GameResult, boolean]>([
    ['a win', { Winner: 0 }, true],
    ['a draw', 'Draw', true],
    ['a win seen without a board', { Winner: 0 }, false],
  ])('offers it for %s: %s', async (_what, result, board) => {
    await ended(result, board)
    expect(screen.queryByRole('button', { name: 'Look at the table and log' }) !== null).toBe(board)
  })
})
