// Goal: every seat sees a die landing on its result when a public roll arrives (a still result under reduced motion), a roll goes by its timer, Dismiss or Escape, and the log names who rolled.
import { act, fireEvent, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { Event } from '@/protocol/generated'
import { batchOf } from '@/test-utils/batches'
import { removeFakeClock, installFakeClock } from '@/test-utils/fakeClock'
import { renderTable } from '@/test-utils/tableHarness'
import { applyBatchToStore } from '../dev/applyBatch'
import { ROLL_MS } from './moments'

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
