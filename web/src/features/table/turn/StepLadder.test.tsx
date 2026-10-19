// Goal: the step ladder in the top bar lights the current step in the colour of its phase (solid on your turn, tinted on another seat's), leads each run of one phase with the phase's icon and names the current one beside the colour, fills the steps behind it, leaves the ones to come hollow, dashes a step the game passed without a window for 1 s before it settles as passed, starts over with each turn, adds first-strike damage only while it matters, is a picture a screen reader skips, and draws the same marks without the timed cues.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { Step } from '@/protocol/generated'
import { cueDom, renderCueTable, useCueClock } from '@/test-utils/cueTable'
import { renderTable } from '@/test-utils/tableHarness'
import { passTurn } from '@/test-utils/turnEvents'
import { buildFixture } from '../dev/fixture'
import { StepLadder } from './StepLadder'
import { SKIPPED_MARK_MS } from './timing'

const ladder = (): HTMLElement | null => document.querySelector('[data-slot="step-ladder"]')
const marks = (): string =>
  ladder() === null
    ? ''
    : cueDom()
        .pips.map((pip) => pip.split(':')[1]?.[0])
        .join('')

beforeEach(useCueClock)
afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

describe('the marks', () => {
  it('lights the current step on an opponent’s turn as a tint with a heavier edge and fills the ones behind it', () => {
    renderCueTable()
    // Bo's turn 5, in declare blockers: Upkeep to Declare attackers are behind, four steps are to come.
    expect(ladder()).toHaveAttribute('data-side', 'them')
    expect(marks()).toBe('pppppcuuuu')
    expect(ladder()?.querySelector('[data-state="current"]')).toHaveClass(
      'bg-phase-soft',
      'border-2',
      'border-phase',
    )
  })

  it('lights it solid in the colour of the phase on your own turn', () => {
    renderCueTable({}, { fixture: buildFixture({ seats: 2, permanents: 2, active: 0 }) })
    expect(ladder()).toHaveAttribute('data-side', 'you')
    const lit = ladder()?.querySelector('[data-state="current"]')
    expect(lit).toHaveClass('bg-phase', 'border-phase')
  })

  it('tells the marks apart by shape: filled, hollow and dashed, not by colour alone', () => {
    renderCueTable()
    const of = (state: string) => ladder()?.querySelector(`[data-state="${state}"]`)
    expect(of('passed')).toHaveClass('bg-step-passed')
    expect(of('upcoming')).not.toHaveClass('bg-step-passed')
    expect(of('upcoming')).toHaveClass('border-border-strong')
    expect(of('current')).toHaveClass('bg-phase-soft')
  })

  it('names each step for a pointer', () => {
    renderCueTable()
    const titles = [...(ladder()?.querySelectorAll('li') ?? [])].map((pip) => pip.getAttribute('title'))
    expect(titles).toEqual([
      'Upkeep',
      'Draw',
      'Precombat main',
      'Beginning of combat',
      'Declare attackers',
      'Declare blockers',
      'Combat damage',
      'End of combat',
      'Postcombat main',
      'End step',
    ])
  })

  it('is a picture: hidden from a screen reader, which has the badge, and with nothing to press', () => {
    renderCueTable()
    expect(ladder()).toHaveAttribute('aria-hidden', 'true')
    expect(ladder()?.querySelector('button, a, input, [tabindex]')).toBeNull()
  })

  it('draws first-strike damage only while it is the step', () => {
    const table = renderCueTable()
    expect(cueDom().pips).toHaveLength(10)
    table.deliver([], { step: 'FirstStrikeDamage' })
    expect(cueDom().pips).toHaveLength(11)
    table.deliver([], { step: 'CombatDamage' })
    expect(cueDom().pips).toHaveLength(10)
  })

  it('is not drawn before the turns begin or after they end', () => {
    const table = renderCueTable()
    table.update((state) => ({ view: state.view && { ...state.view, stage: 'Mulligan' } }))
    expect(ladder()).toBeNull()
    table.update((state) => ({ view: state.view && { ...state.view, stage: 'Finished' } }))
    expect(ladder()).toBeNull()
  })
})

describe('the phases', () => {
  const runs = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>('[data-slot="phase-run"]')]
  const lit = (): number[] =>
    runs().flatMap((run, index) => (run.hasAttribute('data-current') ? [index] : []))

  it('leads each run of one phase with its icon: five runs, the main phase on both sides of combat', () => {
    renderCueTable()
    expect(runs().map((run) => run.dataset.phaseId)).toEqual([
      'beginning',
      'main',
      'combat',
      'main',
      'ending',
    ])
    for (const run of runs()) expect(run.querySelector('svg'), run.dataset.phaseId).not.toBeNull()
  })

  it('names the phase of the current run, and no other, with its icon in the phase’s colour beside the lit step', () => {
    renderCueTable()
    // Bo's turn, declare blockers: the combat run is the one that is lit.
    expect(lit()).toEqual([2])
    expect(runs().map((run) => run.querySelector('span')?.textContent ?? '')).toEqual([
      '',
      '',
      'Combat',
      '',
      '',
    ])
    expect(runs()[2]?.querySelector('svg')).toHaveClass('text-phase')
    expect(runs()[1]?.querySelector('svg')).toHaveClass('text-muted-foreground')
  })

  it.each<[Step, number]>([
    ['Upkeep', 0],
    ['Draw', 0],
    ['Main1', 1],
    ['BeginCombat', 2],
    ['EndCombat', 2],
    ['Main2', 3],
    ['End', 4],
  ])('with the turn in %s it lights run %i of the ladder', (step, run) => {
    const table = renderCueTable()
    table.deliver([], { step })
    expect(lit()).toEqual([run])
  })

  it.each<[Step, number]>([
    ['Untap', 0],
    ['Cleanup', 4],
  ])('with the turn in %s, which the ladder does not draw, it lights run %i of its phase', (step, run) => {
    const table = renderCueTable()
    table.deliver([], { step })
    expect(lit()).toEqual([run])
    expect(document.querySelector('[data-slot="step-ladder"] [data-state="current"]')).toBeNull()
  })

  it('keeps first-strike damage in the combat run while it is drawn', () => {
    const table = renderCueTable()
    table.deliver([], { step: 'FirstStrikeDamage' })
    expect(runs()[2]?.querySelectorAll('li')).toHaveLength(6)
    expect(runs()[2]).toHaveAttribute('data-current')
  })

  it('marks every piece that takes the phase’s colour, so the change between phases can fade', () => {
    renderCueTable()
    for (const part of document.querySelectorAll(
      '[data-slot="step-ladder"] li, [data-slot="phase-run"] svg',
    )) {
      expect(part).toHaveAttribute('data-themed')
    }
  })
})

describe('a skipped step', () => {
  it('is dashed for a second and then settles as passed', () => {
    // On its own, so that no other part of the table's timers count.
    const table = renderCueTable({}, { ui: <StepLadder /> })
    const idle = vi.getTimerCount()
    table.deliver(passTurn(6, 0), { number: 6, active: 0, step: 'Main1' })
    // Alice's turn 6, in her first main phase: Upkeep and Draw went by unasked.
    expect(cueDom().pips.slice(0, 4)).toEqual([
      'Upkeep:skipped',
      'Draw:skipped',
      'Main1:current',
      'BeginCombat:upcoming',
    ])
    table.advance(SKIPPED_MARK_MS - 1)
    expect(cueDom().pips.slice(0, 2)).toEqual(['Upkeep:skipped', 'Draw:skipped'])
    table.advance(1)
    expect(cueDom().pips.slice(0, 3)).toEqual(['Upkeep:passed', 'Draw:passed', 'Main1:current'])
    table.advance(10_000)
    expect(vi.getTimerCount()).toBe(idle)
  })

  it('shows the dash in the shape the stylesheet draws: dashed and hollow', () => {
    const table = renderCueTable()
    table.deliver(passTurn(6, 0), { number: 6, active: 0, step: 'Main1' })
    const dashed = ladder()?.querySelector('[data-state="skipped"]')
    expect(dashed).toHaveClass('border-dashed', 'border-step-skipped')
    expect(dashed).not.toHaveClass('bg-step-passed')
  })

  it('starts the ladder over with the next turn, whatever was dashed before', () => {
    const table = renderCueTable()
    table.deliver(passTurn(6, 0), { number: 6, active: 0, step: 'Main1' })
    table.advance(300)
    table.deliver(passTurn(7, 1), { number: 7, active: 1, step: 'Main1' })
    expect(ladder()).toHaveAttribute('data-side', 'them')
    expect(cueDom().pips.slice(0, 3)).toEqual(['Upkeep:skipped', 'Draw:skipped', 'Main1:current'])
    table.advance(SKIPPED_MARK_MS)
    expect(cueDom().pips.slice(0, 3)).toEqual(['Upkeep:passed', 'Draw:passed', 'Main1:current'])
  })

  it('stretches with the dev slow-down', () => {
    const table = renderCueTable({ effectsSlow: 3 })
    table.deliver(passTurn(6, 0), { number: 6, active: 0, step: 'Main1' })
    table.advance(SKIPPED_MARK_MS * 3 - 1)
    expect(cueDom().pips[0]).toBe('Upkeep:skipped')
    table.advance(1)
    expect(cueDom().pips[0]).toBe('Upkeep:passed')
  })

  it('has no sweep under reduced motion: the same dash, and no motion class to draw it with', () => {
    vi.stubGlobal('matchMedia', (query: string) => ({
      media: query,
      matches: query.includes('reduce'),
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
    }))
    const table = renderCueTable()
    table.deliver(passTurn(6, 0), { number: 6, active: 0, step: 'Main1' })
    expect(cueDom().pips[0]).toBe('Upkeep:skipped')
    for (const pip of ladder()?.querySelectorAll('li') ?? []) {
      expect(pip.className).not.toMatch(/animate|transition|duration/)
    }
  })
})

describe('without the timed cues', () => {
  it('still shows where the turn stands, and never dashes a step', () => {
    vi.useRealTimers()
    const table = renderTable({ seats: 2 })
    expect(marks()).toBe('pppppcuuuu')
    table.update((state) => ({
      view: state.view && { ...state.view, turn: { ...state.view.turn, step: 'Main2' } },
    }))
    expect(cueDom().pips.at(-2)).toBe('Main2:current')
    expect(document.querySelector('[data-state="skipped"]')).toBeNull()
  })
})
