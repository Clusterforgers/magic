// Goal: every pod form (panel, one-line pod, tile) shows each seat's time on its own turns with a clock icon and words, nothing before the seat's first turn, and ticks once a second for the running seat only.
import { act, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { PlayState } from '@/play/store'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from './dev/fixture'
import { SeatPanel } from './SeatPanel'
import { SeatTile } from './SeatTile'
import { CombatProviders } from './combat/CombatProviders'

afterEach(() => vi.useRealTimers())

/** Sets every seat's turn time as (ms, running) and the time the table arrived. */
const setTimes =
  (times: Record<number, [number, boolean]>, at: number) =>
  (state: PlayState): Partial<PlayState> => {
    const view = must(state.view)
    return {
      viewAt: at,
      view: {
        ...view,
        players: view.players.map((player) => {
          const [ms, running] = times[player.seat] ?? [0, false]
          return { ...player, turn_time_ms: ms, turn_time_running: running }
        }),
      },
    }
  }

const marks = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>('[data-slot="turn-time"]')]

describe.each([
  ['panel', (seat: number) => <SeatPanel seat={seat} variant="panel" />],
  ['line', (seat: number) => <SeatPanel seat={seat} variant="line" />],
  ['tile', (seat: number) => <SeatTile seat={seat} focused={false} onFocusSeat={() => undefined} />],
])('the turn time in a %s', (_name, pod) => {
  const show = (seat: number) =>
    renderTable({
      fixture: buildFixture({ seats: 3 }),
      ui: <CombatProviders>{pod(seat)}</CombatProviders>,
    })

  it('is absent before the seat has had a turn', () => {
    const table = show(1)
    table.update(setTimes({}, Date.now()))
    expect(marks()).toHaveLength(0)
  })

  it('shows a finished total as m:ss with its words and does not tick', () => {
    vi.useFakeTimers({ now: 50_000 })
    const table = show(1)
    table.update(setTimes({ 1: [725_000, false], 0: [3_000, true] }, 50_000))
    const mark = must(marks()[0])
    expect(screen.getByRole('img', { name: 'Time used on own turns: 12 minutes 5 seconds' })).toBe(mark)
    expect(mark).toHaveTextContent('12:05')
    expect(mark).not.toHaveAttribute('data-running')
    act(() => vi.advanceTimersByTime(5_000))
    expect(mark).toHaveTextContent('12:05')
  })

  it('stands apart from the name, whose row keeps its room for the name and the life', () => {
    const table = show(1)
    table.update(setTimes({ 1: [725_000, false] }, Date.now()))
    const heading = must(screen.getByText('Bo', { selector: 'h3, span.truncate' }).parentElement)
    expect(heading.contains(must(marks()[0]))).toBe(false)
  })

  it('ticks from the moment the table arrived while the seat plays its turn', () => {
    vi.useFakeTimers({ now: 100_000 })
    const table = show(2)
    table.update(setTimes({ 2: [3_599_000, true] }, 100_000))
    const mark = must(marks()[0])
    expect(mark).toHaveAttribute('data-running', 'true')
    expect(mark).toHaveTextContent('59:59')
    act(() => vi.advanceTimersByTime(2_000))
    expect(mark).toHaveTextContent('1:00:01')
    expect(mark).toHaveAccessibleName('Time used on own turns: 1 hour 0 minutes 1 second')
  })
})
