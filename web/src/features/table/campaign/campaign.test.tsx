// Goal: a campaign fight reads at the table: a boss seat's pod and tile carry the crown chip, the boss edge and a phase track whose steps are passed, now or to come by `begun`; a fallen hero's pod and tile carry the Spirit mark, the ghost at the life figure and "Spirit" in their names; the phase banner shows the phase that began only for a batch that raises `begun`, never for the first view or a snapshot, and is announced politely.
import { act, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { applyBatchToStore } from '@/features/table/dev/applyBatch'
import type { PlayState } from '@/play/store'
import type { BossView, PlayerView, StateView } from '@/protocol/generated'
import { batchOf, clone, must } from '@/test-utils/batches'
import { renderCueTable, useCueClock } from '@/test-utils/cueTable'
import { renderTable } from '@/test-utils/tableHarness'
import { pass } from '@/test-utils/turnEvents'
import { buildFixture } from '../dev/fixture'
import { SeatPanel } from '../SeatPanel'
import { SeatTile } from '../SeatTile'
import { BANNER_TIMES } from '../turn/timing'
import { phaseCueBetween, phaseSteps, phaseTitle, roman } from './bossPhase'
import { PhaseBanner } from './PhaseBanner'

const PHASES = ['Phase II — The Nest Stirs', 'Phase III — Plague Tide']
const boss = (begun: number): BossView => ({ phases: PHASES, begun })

function edited(edit: (view: StateView) => void): (state: PlayState) => PlayState {
  return (state) => {
    const view = clone(must(state.view))
    edit(view)
    return { ...state, view }
  }
}

const seatOf = (view: StateView, seat: number): PlayerView => must(view.players.find((p) => p.seat === seat))

describe('the phase words', () => {
  it.each([
    [0, ['current', 'next', 'next']],
    [1, ['done', 'current', 'next']],
    [2, ['done', 'done', 'current']],
  ])('with %i begun, the steps read %j', (begun, states) => {
    const steps = phaseSteps(boss(begun))
    expect(steps.map((s) => s.state)).toEqual(states)
    expect(steps.map((s) => s.label)).toEqual(['Phase I', 'Phase II', 'Phase III'])
    expect(steps.map((s) => s.title)).toEqual(['', 'The Nest Stirs', 'Plague Tide'])
  })

  it.each([
    ['Phase II — Regrowth', 'Regrowth'],
    ['Wrath', 'Wrath'],
  ])('titles %s as %s', (name, title) => expect(phaseTitle(name)).toBe(title))

  it.each([
    [1, 'I'],
    [4, 'IV'],
    [12, '12'],
  ])('writes %i as %s', (n, text) => expect(roman(n)).toBe(text))

  const player = (seat: number, begun?: number) =>
    ({ seat, ...(begun === undefined ? {} : { boss: boss(begun) }) }) as PlayerView
  const name = (seat: number) => (seat === 1 ? 'Skrix' : 'Ana')

  it.each([
    ['a first view', undefined, [player(1, 1)], null],
    ['an unchanged count', [player(1, 1)], [player(1, 1)], null],
    ['a seat new to the view', [player(0)], [player(0), player(1, 1)], null],
    ['a seat without a boss', [player(0)], [player(0)], null],
    ['a raised count', [player(1, 0)], [player(1, 1)], 'Skrix enters Phase II: The Nest Stirs.'],
    ['two phases at once', [player(1, 0)], [player(1, 2)], 'Skrix enters Phase III: Plague Tide.'],
  ])('gives %s the cue %s', (_, before, after, words) => {
    expect(phaseCueBetween(before, after, name)?.announcement ?? null).toBe(words)
  })
})

describe('the pod and the tile', () => {
  const pod = (seat: number, edit: (view: StateView) => void, variant: 'panel' | 'line' = 'panel') =>
    renderTable({ seats: 4, ui: <SeatPanel seat={seat} variant={variant} />, state: edited(edit) })

  it.each(['panel', 'line'] as const)('frames a boss %s with the crown and its phase track', (variant) => {
    pod(1, (view) => void (seatOf(view, 1).boss = boss(1)), variant)
    const section = must(document.querySelector<HTMLElement>('[data-seat="1"]'))
    expect(section).toHaveAttribute('data-boss', 'true')
    expect(section).toHaveClass('border-boss')
    expect(section.getAttribute('aria-label')).toMatch(/, boss$/)
    expect(section.querySelector('[data-slot="boss-chip"]')).not.toBeNull()
    const steps = [...section.querySelectorAll('[data-slot="boss-track"] li')]
    expect(steps.map((s) => s.getAttribute('data-state'))).toEqual(['done', 'current', 'next'])
    expect(steps[1]).toHaveAttribute('aria-current', 'step')
    expect(screen.getByRole('list', { name: 'Boss phases' })).toHaveTextContent(
      'Phase II: The Nest Stirs, now',
    )
  })

  it.each(['panel', 'line'] as const)('marks a fallen hero’s %s as a Spirit', (variant) => {
    pod(
      2,
      (view) => {
        seatOf(view, 2).spirit = true
        seatOf(view, 2).life = 0
      },
      variant,
    )
    const section = must(document.querySelector<HTMLElement>('[data-seat="2"]'))
    expect(section).toHaveAttribute('data-spirit', 'true')
    expect(section.getAttribute('aria-label')).toMatch(/, Spirit$/)
    expect(section.querySelector('[data-slot="spirit-mark"]')).toHaveTextContent('Spirit')
    expect(section.querySelector('[data-slot="spirit-life"]')).not.toBeNull()
  })

  it('draws no campaign mark for an ordinary seat', () => {
    pod(1, () => undefined)
    const section = must(document.querySelector<HTMLElement>('[data-seat="1"]'))
    expect(section).not.toHaveAttribute('data-boss')
    expect(section).not.toHaveAttribute('data-spirit')
    expect(section.querySelector('[data-slot="boss-track"], [data-slot="spirit-mark"]')).toBeNull()
  })

  it('marks the tile of a boss and of a Spirit, and says Spirit in the tile’s name', () => {
    renderTable({
      fixture: buildFixture({ seats: 5, permanents: 2 }),
      ui: (
        <>
          <SeatTile seat={1} focused={false} onFocusSeat={() => undefined} />
          <SeatTile seat={2} focused={false} onFocusSeat={() => undefined} />
        </>
      ),
      state: edited((view) => {
        seatOf(view, 1).boss = boss(0)
        seatOf(view, 2).spirit = true
      }),
    })
    const bossTile = must(document.querySelector<HTMLElement>('[data-tile-seat="1"]'))
    const spiritTile = must(document.querySelector<HTMLElement>('[data-tile-seat="2"]'))
    expect(bossTile).toHaveAttribute('data-boss', 'true')
    expect(bossTile.querySelector('[data-slot="boss-chip"]')).not.toBeNull()
    expect(spiritTile).toHaveAttribute('data-spirit', 'true')
    expect(spiritTile.getAttribute('aria-label')).toContain('Spirit')
  })
})

describe('the phase banner', () => {
  beforeEach(useCueClock)
  afterEach(() => vi.useRealTimers())

  const banner = () => document.querySelector<HTMLElement>('[data-slot="phase-banner"]')
  const announced = () => document.querySelector('[data-slot="phase-announcement"]')?.textContent

  function setup() {
    const table = renderCueTable({}, { ui: <PhaseBanner /> })
    table.update((state) => {
      const view = clone(must(state.view))
      seatOf(view, 1).boss = boss(0)
      return { view }
    })
    let batches = 0
    const deliver = (begun: number) => {
      const state = table.store.getState()
      const view = clone(must(state.view))
      seatOf(view, 1).boss = boss(begun)
      batches += 1
      view.priority = { holder: batches % 2, passed: [] }
      act(() =>
        applyBatchToStore(table.store, batchOf(view, { from_seq: must(state.seq), events: [pass(0)] })),
      )
    }
    return { table, deliver }
  }

  it('stays away for the first view and for a batch that begins no phase', () => {
    const { deliver } = setup()
    expect(banner()).toBeNull()
    deliver(0)
    expect(banner()).toBeNull()
    expect(announced()).toBe('')
  })

  it('names the phase that began, then fades in the turn banner’s times', () => {
    const { table, deliver } = setup()
    deliver(1)
    expect(banner()).toHaveAttribute('data-phase', 'in')
    expect(banner()).toHaveAttribute('aria-hidden', 'true')
    expect(banner()).toHaveTextContent('Phase II')
    expect(banner()).toHaveTextContent('The Nest Stirs')
    expect(announced()).toMatch(/enters Phase II: The Nest Stirs\.$/)
    table.advance(BANNER_TIMES.inMs)
    expect(banner()).toHaveAttribute('data-phase', 'hold')
    table.advance(BANNER_TIMES.holdMs)
    expect(banner()).toHaveAttribute('data-phase', 'out')
    table.advance(BANNER_TIMES.outMs)
    expect(banner()).toBeNull()
  })

  it('stays away for a snapshot that moves the phase', () => {
    const { table } = setup()
    table.update((state) => {
      const view = clone(must(state.view))
      seatOf(view, 1).boss = boss(2)
      return { view, seq: (state.seq ?? 0) + 40 }
    })
    expect(banner()).toBeNull()
  })
})
