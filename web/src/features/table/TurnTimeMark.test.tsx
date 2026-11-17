// Goal: every pod form (panel, one-line pod, tile) shows each seat's time on its own turns with a clock icon and words, nothing before the seat's first turn, and ticks once a second for the running seat only; under shared team turns (Two-Headed Giant, shared turns, a party) the seat the table names ticks, a teammate of the team's first seat included, and the first seat does not tick for it.
import { act, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { PlayState } from '@/play/store'
import type { Teams } from '@/protocol/generated'
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

/** The mark of one seat's pod. */
const markOf = (seat: number): HTMLElement =>
  must(document.querySelector<HTMLElement>(`[data-seat="${seat}"][data-variant] [data-slot="turn-time"]`))

/** Gives the table `teams` and the turn to seat 0, the first seat of its team. */
const teamTurn =
  (teams: Teams) =>
  (state: PlayState): PlayState => {
    const view = structuredClone(must(state.view))
    view.house_rules = { teams }
    view.turn = { ...view.turn, active: 0 }
    return { ...state, view }
  }

describe.each<[string, Teams]>([
  ['Two-Headed Giant', { mode: 'two_headed_giant', seats: [0, 0, 1, 1] }],
  ['shared team turns', { mode: 'shared_turns', seats: [0, 0, 1, 1] }],
  ['a party of three', { mode: 'shared_turns', seats: [0, 0, 0, 1] }],
])('the turn time under %s', (_name, teams) => {
  const show = () =>
    renderTable({
      fixture: buildFixture({ seats: 4, active: 0, viewer: 3 }),
      state: teamTurn(teams),
      ui: (
        <CombatProviders>
          {[0, 1, 2].map((seat) => (
            <SeatPanel key={seat} seat={seat} variant="line" />
          ))}
        </CombatProviders>
      ),
    })

  it("ticks the teammate whose decision the game waits for and never the team's first seat", () => {
    vi.useFakeTimers({ now: 100_000 })
    const table = show()
    table.update(setTimes({ 0: [3_000, false], 1: [5_000, true], 2: [0, false] }, 100_000))
    expect(markOf(0)).not.toHaveAttribute('data-running')
    expect(markOf(1)).toHaveAttribute('data-running', 'true')
    act(() => vi.advanceTimersByTime(2_000))
    expect(markOf(0)).toHaveTextContent('0:03')
    expect(markOf(1)).toHaveTextContent('0:07')
  })

  it('moves the ticking to the seat the next table names and stops the one that answered', () => {
    vi.useFakeTimers({ now: 100_000 })
    const table = show()
    table.update(setTimes({ 0: [3_000, false], 1: [5_000, true] }, 100_000))
    act(() => vi.advanceTimersByTime(2_000))
    table.update(setTimes({ 0: [3_000, true], 1: [7_000, false] }, 102_000))
    expect(markOf(0)).toHaveAttribute('data-running', 'true')
    expect(markOf(1)).not.toHaveAttribute('data-running')
    act(() => vi.advanceTimersByTime(4_000))
    expect(markOf(0)).toHaveTextContent('0:07')
    expect(markOf(1)).toHaveTextContent('0:07')
  })

  it('ticks every seat that is asked together and no seat while an opponent decides', () => {
    vi.useFakeTimers({ now: 100_000 })
    const table = show()
    table.update(setTimes({ 0: [1_000, true], 1: [2_000, true], 2: [4_000, false] }, 100_000))
    act(() => vi.advanceTimersByTime(3_000))
    expect([0, 1, 2].map((seat) => markOf(seat).textContent)).toEqual(['0:04', '0:05', '0:04'])
    table.update(setTimes({ 0: [4_000, false], 1: [5_000, false], 2: [4_000, false] }, 103_000))
    act(() => vi.advanceTimersByTime(5_000))
    expect([0, 1, 2].map((seat) => markOf(seat).textContent)).toEqual(['0:04', '0:05', '0:04'])
    expect(document.querySelectorAll('[data-slot="turn-time"][data-running]')).toHaveLength(0)
  })
})
