/** A table with the turn cues on and a manual clock, and the means to hand it a batch the way the server would. */
import { act } from '@testing-library/react'
import { vi } from 'vitest'
import { applyBatchToStore } from '@/features/table/dev/applyBatch'
import { buildFixture } from '@/features/table/dev/fixture'
import type { TableOptions } from '@/features/table/tableOptions'
import type { EventView } from '@/protocol/generated'
import { batchOf, clone, must } from './batches'
import { renderTable } from './tableHarness'
import type { TableHarness, TableSetup } from './tableHarness'
import type { TurnAt } from './turnEvents'

/** A table showing the cues, and `deliver` to apply a batch that leaves the game at `to`. */
export interface CueTable extends TableHarness {
  /** Applies a batch of `events` after which the turn is where `to` says (what it does not name stays as it was). */
  deliver(events: readonly EventView[], to?: Partial<TurnAt>): void
  /** Moves the manual clock, inside `act`. */
  advance(ms: number): void
}

/** Starts the manual clock; call it in `beforeEach` and `vi.useRealTimers()` in `afterEach`. */
export function useCueClock(): void {
  vi.useFakeTimers()
}

/** A two-seat table at Bo's turn 5 (seat 0 views it) with the turn cues on; `setup` and `options` change it. */
export function renderCueTable(options: TableOptions = {}, setup: TableSetup = {}): CueTable {
  const harness = renderTable({
    fixture: buildFixture({ seats: 2, permanents: 2 }),
    ...setup,
    options: { cues: true, ...options },
  })
  const { store } = harness
  let batches = 0
  return {
    ...harness,
    deliver(events, to = {}) {
      const state = store.getState()
      const view = clone(must(state.view))
      view.turn = { ...view.turn, ...to }
      // A batch always moves something in the table; a store sees an equal one as no change, so priority alternates.
      batches += 1
      view.priority = { holder: batches % 2, passed: [] }
      act(() => {
        applyBatchToStore(store, batchOf(view, { from_seq: must(state.seq), events: [...events] }))
      })
    },
    advance(ms) {
      act(() => {
        vi.advanceTimersByTime(ms)
      })
    },
  }
}

/** What the cues show now, as plain text and attributes. */
export function cueDom() {
  const one = (selector: string): HTMLElement | null => document.querySelector<HTMLElement>(selector)
  const banner = one('[data-slot="turn-banner"]')
  return {
    phase: banner?.getAttribute('data-phase') ?? null,
    banner: banner?.textContent ?? null,
    announcement: one('[data-slot="turn-announcement"]')?.textContent ?? null,
    ticker: one('[data-slot="turn-ticker-line"]')?.textContent ?? null,
    pips: [...document.querySelectorAll<HTMLElement>('[data-slot="step-ladder"] li')].map(
      (pip) => `${pip.dataset.step}:${pip.dataset.state}`,
    ),
  }
}
