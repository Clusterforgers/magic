// Goal: a watch link's page connects to the spectator's address, draws the table of a running game with no seat of its own and no pass controls, tells a spectator whom the host stopped that the game cannot be watched now, and a watch address without a token opens nothing.
import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { StateView } from '@/protocol/generated'
import { clone } from '@/test-utils/batches'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { GAME_ID, renderGame } from '@/test-utils/gameHarness'
import { renderInRouter } from '@/test-utils/router'
import { lateJoin } from '@/test-utils/transcripts'
import { WatchPage } from './WatchRoute'

const names = {
  started: true,
  seats: [
    { seat: 0, name: 'Ana' },
    { seat: 1, name: 'Bo' },
  ],
  connected: [0, 1],
  result: null,
}

/** The transcript's view as a spectator gets it: no seat, no hand listed. */
function spectatorView(): StateView {
  const view = clone(lateJoin.expected)
  view.viewer = view.players.length
  view.spectator = true
  for (const player of view.players) player.hand.cards = null
  return view
}

async function watching() {
  const h = await renderGame({ watch: 'watch-token', script: { game: () => jsonResponse(200, names) } })
  h.join(2)
  h.deliver({ type: 'Snapshot', seq: lateJoin.expected.players.length, state: spectatorView() })
  await h.settle()
  return h
}

describe('watching a game', () => {
  it('connects to the spectator address and draws the table with no seat of its own', async () => {
    const h = await watching()
    expect(h.socket().url).toBe(`ws://localhost:5173/ws/${GAME_ID}/watch/watch-token`)
    expect(screen.getByRole('region', { name: 'Game table' })).toBeInTheDocument()
    expect(screen.getByText('Watching: you hold no seat')).toBeInTheDocument()
    expect(h.socket().sentTypes, 'a spectator sends nothing but its Join').toEqual(['Join'])
  })

  it('stops for good when the host stops spectators', async () => {
    const h = await watching()
    h.close(4001, 'removed from the game')
    expect(await screen.findByText(/The host stopped spectators/)).toBeInTheDocument()
  })

  it('opens nothing from a watch address without a token', async () => {
    await renderInRouter(<WatchPage gameId={GAME_ID} fragment="seat=0" />)
    expect(await screen.findByText('This watch link is incomplete')).toBeInTheDocument()
  })
})
