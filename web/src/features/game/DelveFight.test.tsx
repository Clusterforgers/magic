// Goal: a Delves boss fight has its own waiting room in place of the game's lobby (the boss intro with its abilities, the party's life, one Ready to fight button and no deck choice), and when the fight ends the page goes straight back to its run, with no game-over screen.
import { screen, waitFor, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { delveView, fakeDelves } from '@/features/delves/fakeDelves'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { renderGame, GAME_ID } from '@/test-utils/gameHarness'
import { lobbyMessage } from '@/test-utils/lobbyFrames'
import type { SeatFlags } from '@/test-utils/lobbyFrames'

const TABLE: SeatFlags[] = [
  ['Ana', true, false],
  ['Skrix, the Rat Queen', true, true],
]

const fight = {
  boss: 'Skrix, the Rat Queen',
  life: 120,
  rules: ['skrix'],
  phases: [['nest', 80]] as Array<[string, number]>,
  minions: [],
}

function info(result: unknown = null) {
  return {
    format: 'commander',
    started: false,
    seats: TABLE.map(([name], seat) => ({ seat, name })),
    connected: [0, 1],
    ready: [1],
    result,
    visibility: 'invite',
    manual_play: false,
    delve: 'd1',
  }
}

async function fightPage() {
  const delves = fakeDelves(
    delveView({ boss: 'skrix_nest', fight }, { status: { Fight: { game: GAME_ID } }, at: 0 }),
  )
  const h = await renderGame({ routed: true, delves, script: { game: () => jsonResponse(200, info()) } })
  h.join(0, TABLE.length)
  h.deliver(lobbyMessage(TABLE))
  await h.settle()
  return h
}

describe('a Delves boss fight', () => {
  it('waits in the boss intro with Ready to fight and no deck choice', async () => {
    const h = await fightPage()
    const intro = await screen.findByRole('region', { name: 'Boss fight' })
    expect(within(intro).getByRole('heading', { name: 'Skrix, the Rat Queen' })).toBeInTheDocument()
    expect(within(intro).getByRole('list', { name: 'Boss abilities' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /Use this deck|Choose a deck/ })).toBeNull()
    expect(screen.queryByRole('listbox')).toBeNull()
    await h.user.click(screen.getByRole('button', { name: 'Ready to fight' }))
    expect(h.socket().sentTypes.filter((type) => type !== 'SetStop')).toEqual(['Join', 'Ready'])
    expect(screen.getByRole('link', { name: 'Back to the run' })).toHaveAttribute('href', '/delves/d1')
  })

  it('goes back to the run when the fight ends', async () => {
    const h = await fightPage()
    await screen.findByRole('region', { name: 'Boss fight' })
    h.deliver({ type: 'GameOver', result: { Team: 0 } })
    await h.settle()
    await waitFor(() => expect(h.router?.state.location.pathname).toBe('/delves/d1'))
    expect(screen.queryByText(/Game over/)).toBeNull()
  })
})
