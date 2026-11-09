// Goal: every seat sees a die landing on its result, and a coin turning over to its side with the call and whether it won, when a public roll or flip arrives (a still result under reduced motion); a moment goes by its timer, Dismiss or Escape, and the log names who rolled.
import { act, fireEvent, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { CoinSide, Event } from '@/protocol/generated'
import { batchOf } from '@/test-utils/batches'
import { removeFakeClock, installFakeClock } from '@/test-utils/fakeClock'
import { renderTable } from '@/test-utils/tableHarness'
import { applyBatchToStore } from '../dev/applyBatch'
import { COIN_FLIP_MS, ROLL_MS } from './moments'

beforeEach(installFakeClock)
afterEach(removeFakeClock)

function rolled(event: Event) {
  const table = renderTable({ seats: 2, options: { effects: true } })
  const state = table.store.getState()
  const view = state.view
  if (view === null) throw new Error('no view')
  act(() => {
    const batch = batchOf(view, { from_seq: state.seq ?? 0, events: [{ Public: event }] })
    // A batch that changes nothing in the view is not told apart from a decision, so the turn moves on.
    batch.table.turn.number += 1
    applyBatchToStore(table.store, batch)
  })
  return table
}

const ROLLS: readonly [Event, RegExp][] = [
  [{ DieRolled: { player: 1, sides: 20, result: 17 } }, /rolls a d20/],
  [{ ManualRolled: { player: 0, kind: { Die: { sides: 6 } }, result: 4 } }, /rolls a d6/],
]

describe('the moment of a die', () => {
  it.each(ROLLS)('%j shows a panel that goes after its time', (event, title) => {
    rolled(event)
    expect(screen.getByRole('region', { name: title })).toHaveAttribute('data-kind', 'roll')
    act(() => void vi.advanceTimersByTime(ROLL_MS + 10))
    expect(screen.queryByRole('region', { name: title })).toBeNull()
  })

  it.each(ROLLS)('%j ends with Escape', (event, title) => {
    rolled(event)
    fireEvent.keyDown(document, { key: 'Escape' })
    expect(screen.queryByRole('region', { name: title })).toBeNull()
  })

  it.each(ROLLS)('%j ends with Dismiss', (event, title) => {
    rolled(event)
    fireEvent.click(screen.getByRole('button', { name: 'Dismiss' }))
    expect(screen.queryByRole('region', { name: title })).toBeNull()
  })
})

const flipped = (call: CoinSide, side: CoinSide): Event => ({
  CoinFlipped: { player: 1, won: call === side, call, side },
})

describe('the moment of a coin', () => {
  afterEach(() => void vi.unstubAllGlobals())

  it.each([
    ['Heads', 'Heads', 'wins'],
    ['Heads', 'Tails', 'loses'],
    ['Tails', 'Heads', 'loses'],
    ['Tails', 'Tails', 'wins'],
  ] as const)('called %s, it turns, lands on %s and the caller %s', (call, side, result) => {
    rolled(flipped(call, side))
    const panel = screen.getByRole('region', { name: /flips a coin/ })
    expect(panel).toHaveAttribute('data-kind', 'coin')
    const coin = panel.querySelector('[data-slot="coin"]')
    expect(coin).toHaveAttribute('data-side', side)
    expect(coin).not.toHaveAttribute('data-landed')
    expect(panel).toHaveTextContent(`Called ${call.toLowerCase()}`)
    expect(panel).not.toHaveTextContent(result)
    act(() => void vi.advanceTimersByTime(COIN_FLIP_MS + 10))
    expect(coin).toHaveAttribute('data-landed')
    expect(panel.querySelector('[data-slot="coin-side"]')).toHaveTextContent(side)
    expect(panel).toHaveTextContent(`Called ${call.toLowerCase()} · ${result}`)
    act(() => void vi.advanceTimersByTime(ROLL_MS))
    expect(screen.queryByRole('region', { name: /flips a coin/ })).toBeNull()
  })

  it('under reduced motion shows the side at once and does not turn', () => {
    vi.stubGlobal('matchMedia', (query: string) => ({
      media: query,
      matches: query.includes('prefers-reduced-motion: reduce'),
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
    }))
    rolled(flipped('Tails', 'Heads'))
    const panel = screen.getByRole('region', { name: /flips a coin/ })
    const coin = panel.querySelector('[data-slot="coin"]')
    expect(coin).toHaveAttribute('data-still')
    expect(coin).toHaveAttribute('data-landed')
    expect(panel).toHaveTextContent('Called tails · loses')
  })

  it('a coin flipped by hand has no call', () => {
    rolled({ ManualRolled: { player: 0, kind: { Coin: {} }, result: 2 } })
    const panel = screen.getByRole('region', { name: /flips a coin/ })
    expect(panel.querySelector('[data-slot="coin-call"]')).toBeNull()
    act(() => void vi.advanceTimersByTime(COIN_FLIP_MS + 10))
    expect(panel.querySelector('[data-slot="coin-side"]')).toHaveTextContent('Tails')
  })
})
