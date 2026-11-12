// Goal: a spectator's table is drawn from the first seat's side with that hand face down and a "Watching" note in place of the pass controls, and a player's top bar shows how many watch (in words for a screen reader) while some do and nothing otherwise.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { renderTable } from '@/test-utils/tableHarness'
import type { PlayState } from '@/play/store'
import { watchingText } from './SpectatorCount'

/** The fixture as a spectator of its seats sees it: no seat of its own, no hand listed, no decision. */
function spectating(state: PlayState): PlayState {
  const view = state.view
  if (view === null) return state
  const seats = view.players.length
  return {
    ...state,
    seat: seats,
    pending: null,
    spectators: 2,
    view: {
      ...view,
      spectator: true,
      players: view.players.map((player) => ({ ...player, hand: { count: player.hand.count, cards: null } })),
    },
  }
}

describe('a spectator’s table', () => {
  it('says it is watching instead of the pass controls and shows the first seat’s hand face down', () => {
    const harness = renderTable({ state: spectating })
    expect(screen.getByText('Watching: you hold no seat')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /^Pass|^Resolve|^End turn/ })).toBeNull()
    const hand = harness.container.querySelector('[data-region="hand"]') as HTMLElement
    expect(within(hand).queryAllByRole('button', { name: /^Play|^Cast/ })).toHaveLength(0)
    expect(harness.container.querySelector('[data-slot="spectator-count"]')).toBeNull()
  })
})

describe('the spectator count of a player', () => {
  it('shows nothing while nobody watches', () => {
    const harness = renderTable()
    expect(harness.container.querySelector('[data-slot="spectator-count"]')).toBeNull()
  })

  it.each([
    [1, '1 person is watching'],
    [3, '3 people are watching'],
  ])('shows %i spectators as "%s" and follows the count', (count, words) => {
    const harness = renderTable({ state: (state) => ({ ...state, spectators: count }) })
    const chip = screen.getByRole('status', { name: words })
    expect(chip).toHaveTextContent(String(count))
    expect(watchingText(count)).toBe(words)
    harness.update(() => ({ spectators: 0 }))
    expect(harness.container.querySelector('[data-slot="spectator-count"]')).toBeNull()
  })
})
