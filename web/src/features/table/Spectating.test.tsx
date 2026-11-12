// Goal: a spectator's table is drawn from the first seat's side with that hand face down and a "Watching" note in place of the pass controls, offers no spectator menu, and a player's menu copies a new watch link while only the host stops or allows spectators.
import { screen, waitFor, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { renderTable } from '@/test-utils/tableHarness'
import type { PlayState } from '@/play/store'

/** The fixture as a spectator of its seats sees it: no seat of its own, no hand listed, no decision. */
function spectating(state: PlayState): PlayState {
  const view = state.view
  if (view === null) return state
  const seats = view.players.length
  return {
    ...state,
    seat: seats,
    pending: null,
    view: {
      ...view,
      spectator: true,
      players: view.players.map((player) => ({ ...player, hand: { count: player.hand.count, cards: null } })),
    },
  }
}

const calls = () => ({
  watchLink: vi.fn(() => Promise.resolve('https://play.example/watch/g#token=w')),
  setAllowed: vi.fn((allowed: boolean) => Promise.resolve(allowed)),
})

describe('a spectator’s table', () => {
  it('says it is watching instead of the pass controls and shows the first seat’s hand face down', () => {
    const harness = renderTable({ state: spectating })
    expect(screen.getByText('Watching: you hold no seat')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /^Pass|^Resolve|^End turn/ })).toBeNull()
    const hand = harness.container.querySelector('[data-region="hand"]') as HTMLElement
    expect(within(hand).queryAllByRole('button', { name: /^Play|^Cast/ })).toHaveLength(0)
  })

  it('offers no spectator menu', async () => {
    const harness = renderTable({ state: spectating, spectators: calls() })
    await harness.user.click(screen.getByRole('button', { name: 'Table menu' }))
    expect(screen.queryByRole('button', { name: 'Copy watch link' })).toBeNull()
  })
})

describe('the spectator menu of a player', () => {
  it('copies a new watch link and says so', async () => {
    const spectators = calls()
    const harness = renderTable({ spectators })
    await harness.user.click(screen.getByRole('button', { name: 'Table menu' }))
    await harness.user.click(screen.getByRole('button', { name: 'Copy watch link' }))
    expect(await screen.findByText(/Watch link copied/)).toBeInTheDocument()
    expect(spectators.watchLink).toHaveBeenCalledTimes(1)
    await waitFor(async () =>
      expect(await navigator.clipboard.readText()).toBe('https://play.example/watch/g#token=w'),
    )
  })

  it('lets the host stop spectators and allow them again', async () => {
    const spectators = calls()
    const harness = renderTable({ spectators })
    await harness.user.click(screen.getByRole('button', { name: 'Table menu' }))
    await harness.user.click(screen.getByRole('button', { name: 'Stop spectators' }))
    expect(await screen.findByText(/Spectators are stopped/)).toBeInTheDocument()
    expect(spectators.setAllowed).toHaveBeenLastCalledWith(false)
    expect(screen.getByRole('button', { name: 'Copy watch link' })).toBeDisabled()
    await harness.user.click(screen.getByRole('button', { name: 'Allow spectators' }))
    expect(await screen.findByText('Spectators are allowed.')).toBeInTheDocument()
    expect(spectators.setAllowed).toHaveBeenLastCalledWith(true)
  })

  it('gives a player who is not the host the link alone', async () => {
    const harness = renderTable({ spectators: calls(), state: (state) => ({ ...state, seat: 1 }) })
    await harness.user.click(screen.getByRole('button', { name: 'Table menu' }))
    expect(screen.getByRole('button', { name: 'Copy watch link' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /spectators$/ })).toBeNull()
  })
})
