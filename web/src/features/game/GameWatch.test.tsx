// Goal: a signed-in spectator's page connects to `/ws/{game}/watch`, sends nothing but its Join and draws the table of a running game with no seat of its own; a visitor not signed in is asked to sign in and come back (and told a server without accounts has no spectators); and a player's page reads the server's spectator count into the top bar.
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

describe('watching a game', () => {
  it('connects to the spectator address and draws the table with no seat of its own', async () => {
    const h = await renderGame({ watch: true, link: null, script: { game: () => jsonResponse(200, names) } })
    expect(h.socket().url).toBe(`ws://localhost:5173/ws/${GAME_ID}/watch`)
    h.join(2)
    h.deliver({ type: 'Snapshot', seq: 0, state: spectatorView() })
    await h.settle()
    expect(screen.getByRole('region', { name: 'Game table' })).toBeInTheDocument()
    expect(screen.getByText('Watching: you hold no seat')).toBeInTheDocument()
    expect(h.socket().sentTypes, 'a spectator sends nothing but its Join').toEqual(['Join'])
  })

  it.each([
    ['signed-out', 'Anyone signed in may watch', true],
    ['unavailable', 'runs without accounts', false],
  ] as const)('asks a visitor who is %s to sign in first', async (status, words, link) => {
    await renderInRouter(<WatchPage gameId={GAME_ID} status={status} />)
    expect(screen.getByRole('heading', { name: 'Sign in to watch this game' })).toBeInTheDocument()
    expect(screen.getByText(new RegExp(words))).toBeInTheDocument()
    const signIn = screen.queryByRole('link', { name: 'Sign in' })
    if (link) expect(signIn).toHaveAttribute('href', `/login?next=%2Fwatch%2F${GAME_ID}`)
    else expect(signIn).toBeNull()
  })
})

describe('a player’s page', () => {
  it('shows how many watch as the server tells it', async () => {
    const h = await renderGame({
      link: { seat: 0, token: 'tok' },
      script: { game: () => jsonResponse(200, names) },
    })
    h.join(0)
    h.deliver({ type: 'Snapshot', seq: 0, state: clone(lateJoin.expected) }, { type: 'Spectators', count: 2 })
    await h.settle()
    expect(screen.getByRole('status', { name: '2 people are watching' })).toBeInTheDocument()
    h.deliver({ type: 'Spectators', count: 0 })
    expect(screen.queryByRole('status', { name: /watching/ })).toBeNull()
  })
})
