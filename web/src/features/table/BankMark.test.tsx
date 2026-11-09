// Goal: a seat's timer bank shows beside its name in every pod form (panel, one-line pod, tile), says nothing while empty, reads in words, and turns to "used" for a few seconds when the count drops so every seat sees a spent extension.
import { act, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { withBank } from '@/play/view'
import type { PlayState } from '@/play/store'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from './dev/fixture'
import { SPENT_MS } from './BankMark'
import { bankText } from './bankText'
import { SeatPanel } from './SeatPanel'
import { SeatTile } from './SeatTile'
import { CombatProviders } from './combat/CombatProviders'

afterEach(() => vi.useRealTimers())

const setBank =
  (seat: number, bank: number) =>
  (state: PlayState): Partial<PlayState> => ({ view: withBank(must(state.view), seat, bank) })

const marks = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>('[data-slot="bank-mark"]')]

describe('bankText', () => {
  it.each([
    [1, false, '1 clock extension left'],
    [3, false, '3 clock extensions left'],
    [0, true, 'Clock extension used, 0 clock extensions left'],
  ])('bank %i spent %s reads %s', (bank, spent, words) => {
    expect(bankText(bank, spent)).toBe(words)
  })
})

describe.each([
  ['panel', (seat: number) => <SeatPanel seat={seat} variant="panel" />],
  ['line', (seat: number) => <SeatPanel seat={seat} variant="line" />],
  ['tile', (seat: number) => <SeatTile seat={seat} focused={false} onFocusSeat={() => undefined} />],
])('the bank mark in a %s', (_name, pod) => {
  const show = () =>
    renderTable({
      fixture: buildFixture({ seats: 3 }),
      ui: <CombatProviders>{pod(1)}</CombatProviders>,
    })

  it('is absent while the bank is empty', () => {
    show()
    expect(marks()).toHaveLength(0)
  })

  it('shows the count with its words once the seat has earned an extension, and goes when none is left', () => {
    vi.useFakeTimers()
    const table = show()
    table.update(setBank(1, 2))
    expect(screen.getByRole('img', { name: '2 clock extensions left' })).toHaveAttribute('data-bank', '2')
    expect(marks()[0]).not.toHaveAttribute('data-spent')
  })

  it('says "used" for a few seconds after the count drops, then goes quiet again', () => {
    vi.useFakeTimers()
    const table = show()
    table.update(setBank(1, 1))
    table.update(setBank(1, 0))
    const mark = must(marks()[0])
    expect(mark).toHaveAttribute('data-spent', 'true')
    expect(mark).toHaveAccessibleName('Clock extension used, 0 clock extensions left')
    act(() => vi.advanceTimersByTime(SPENT_MS))
    expect(marks()).toHaveLength(0)
  })

  it('does not mark a seat whose count rose, and leaves the other seats alone', () => {
    vi.useFakeTimers()
    const table = show()
    table.update(setBank(1, 1))
    table.update(setBank(1, 3))
    table.update(setBank(0, 2))
    expect(marks()).toHaveLength(1)
    expect(marks()[0]).not.toHaveAttribute('data-spent')
  })
})
