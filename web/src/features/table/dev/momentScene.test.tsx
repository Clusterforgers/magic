// Goal: the dev table's moments batch follows on from the table, names a reveal by the first opponent and the viewer's d20, and puts a reveal and a die on show over the table at any seat count.
import { act, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { createPlayStore } from '@/play/store'
import { installFakeClock, removeFakeClock } from '@/test-utils/fakeClock'
import { renderTable } from '@/test-utils/tableHarness'
import { applyBatchToStore } from './applyBatch'
import { momentsBatch, REVEALED_CARDS } from './momentScene'

beforeEach(installFakeClock)
afterEach(removeFakeClock)

describe('the moments of the dev table', () => {
  it('does nothing without a table', () => {
    expect(momentsBatch(createPlayStore().getState())).toBeNull()
  })

  it.each([2, 4, 12])('at %i seats it reveals cards and rolls a die for every seat to see', (seats) => {
    const table = renderTable({ seats, options: { effects: true } })
    const batch = momentsBatch(table.store.getState())
    if (batch === null) throw new Error('no batch')
    expect(batch.shown).toHaveLength(REVEALED_CARDS)
    expect(batch.from_seq).toBe(table.store.getState().seq)
    act(() => void applyBatchToStore(table.store, batch))
    expect(screen.getByRole('region', { name: /reveals$/ })).toHaveAttribute('data-kind', 'reveal')
    expect(screen.getByRole('region', { name: /rolls a d20$/ })).toHaveAttribute('data-kind', 'roll')
  })
})
