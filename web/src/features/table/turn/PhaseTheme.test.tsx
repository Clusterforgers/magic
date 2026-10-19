// Goal: the table root carries the phase of the turn as `data-phase`, following the step the moment it changes and for every step, absent before the turns begin and after they end and the same for every seat; the pieces that take the theme (the bar's rule, the badge, the ladder, the strip's edge) are marked so their colour can fade and name no phase of their own; the badge shows the phase's name and icon beside its colour; and nothing waits, so a theme adds no delay to an answer.
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { PlayState } from '@/play/store'
import type { Step } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { renderCueTable, useCueClock } from '@/test-utils/cueTable'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from '../dev/fixture'
import { STEP_LABEL } from '../turnText'
import { phaseOf } from './phase'

const STEPS = Object.keys(STEP_LABEL) as Step[]
const root = (): HTMLElement => must(document.querySelector<HTMLElement>('[data-slot="table-grid"]'))
const tag = (): HTMLElement | null => document.querySelector<HTMLElement>('[data-slot="phase-tag"]')
const words = (): HTMLElement =>
  must(document.querySelector<HTMLElement>('[data-slot="banner"] [data-slot="pill"]'))
const setStep = (step: Step) => (state: PlayState) => {
  const view = clone(must(state.view))
  view.turn = { ...view.turn, step }
  return { view }
}

afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

describe('the root attribute', () => {
  it.each(STEPS)('is the phase of %s', (step) => {
    const { update } = renderTable({ seats: 2 })
    update(setStep(step))
    expect(root()).toHaveAttribute('data-phase', phaseOf(step).id)
  })

  it('follows the turn from step to step, one theme after the other', () => {
    const { update } = renderTable({ seats: 2 })
    const seen: string[] = []
    for (const step of STEPS) {
      update(setStep(step))
      const phase = root().getAttribute('data-phase') ?? ''
      if (seen.at(-1) !== phase) seen.push(phase)
    }
    expect(seen).toEqual(['beginning', 'main', 'combat', 'main', 'ending'])
  })

  it.each(['PreGame', 'Mulligan', 'Finished'] as const)('is absent while the stage is %s', (stage) => {
    const { update } = renderTable({ seats: 2 })
    expect(root()).toHaveAttribute('data-phase')
    update((state) => ({ view: state.view && { ...state.view, stage } }))
    expect(root()).not.toHaveAttribute('data-phase')
    expect(tag()).toBeNull()
  })

  it.each([0, 1, 2])('is the same for seat %i, which sees the same step as every other', (viewer) => {
    renderTable({ fixture: buildFixture({ seats: 3, viewer, permanents: 2 }) })
    expect(root()).toHaveAttribute('data-phase', 'combat')
    expect(tag()).toHaveTextContent('Combat')
  })

  it('changes in the same batch as the step, with the timed cues on and the clock stopped', () => {
    useCueClock()
    const table = renderCueTable()
    expect(root()).toHaveAttribute('data-phase', 'combat')
    table.deliver([], { step: 'Main2' })
    expect(root()).toHaveAttribute('data-phase', 'main')
    table.deliver([], { step: 'End' })
    expect(root()).toHaveAttribute('data-phase', 'ending')
    expect(tag()).toHaveTextContent('Ending')
  })

  it('is no different under reduced motion: the same attribute at once, and no animation on a piece that fades', () => {
    vi.stubGlobal('matchMedia', (query: string) => ({
      media: query,
      matches: query.includes('reduce'),
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
    }))
    const { update } = renderTable({ seats: 2 })
    update(setStep('Upkeep'))
    expect(root()).toHaveAttribute('data-phase', 'beginning')
    expect(document.querySelectorAll('[data-themed]').length).toBeGreaterThan(0)
    for (const part of document.querySelectorAll('[data-themed]')) {
      // The fade itself is the stylesheet's, and it is off there under reduced motion (`phaseCss.test.ts`).
      expect(part.getAttribute('class') ?? '').not.toMatch(/animate-|motion-/)
    }
  })
})

describe('the badge', () => {
  it.each(STEPS)('shows the name and an icon of the phase of %s, beside its colour', (step) => {
    const { update } = renderTable({ seats: 2 })
    update(setStep(step))
    const phase = phaseOf(step)
    expect(tag()).toHaveTextContent(phase.name)
    expect(tag()?.querySelector('svg')).not.toBeNull()
    expect(tag()).toHaveAttribute('data-phase-id', phase.id)
  })

  it('fills the tag and the words with the accent on your own turn', () => {
    renderTable({ fixture: buildFixture({ seats: 2, active: 0, permanents: 2 }) })
    expect(tag()).toHaveClass('bg-phase', 'text-on-phase', 'border-phase')
    expect(words()).toHaveClass('bg-phase', 'text-on-phase', 'border-phase')
    expect(tag()?.querySelector('svg')).toHaveClass('text-on-phase')
  })

  it('tints the tag and the words on another seat’s turn, with the icon in the accent', () => {
    renderTable({ seats: 2 })
    expect(tag()).toHaveClass('bg-phase-soft', 'text-foreground', 'border-phase')
    expect(words()).toHaveClass('bg-phase-soft', 'text-foreground', 'border-phase')
    expect(tag()?.querySelector('svg')).toHaveClass('text-phase')
  })

  it('is a picture for the eye: hidden from a screen reader, which has the words for the step', () => {
    renderTable({ seats: 2 })
    expect(tag()).toHaveAttribute('aria-hidden', 'true')
    expect(document.querySelector('[data-slot="banner"]')).toHaveTextContent(
      "Bo's turn · 5 · Declare blockers",
    )
    expect(tag()?.closest('[aria-live]')).toBeNull()
  })

  it('leaves the words neutral while no turn is running, with no tag and no phase colour', () => {
    const { update } = renderTable({ seats: 2 })
    update((state) => ({ view: state.view && { ...state.view, stage: 'Mulligan' } }))
    expect(words()).not.toHaveClass('bg-phase', 'bg-phase-soft', 'border-phase')
    expect(words()).not.toHaveAttribute('data-themed')
    expect(tag()).toBeNull()
  })
})

describe('the pieces that take the theme', () => {
  const marked = (selector: string): HTMLElement =>
    must(document.querySelector<HTMLElement>(`${selector}[data-themed]`), selector)

  it('are the bar, the tag, the words of the badge, the ladder’s marks and the strip, each marked to fade', () => {
    renderTable({ seats: 2 })
    for (const selector of [
      '[data-region="bar"]',
      '[data-slot="phase-tag"]',
      '[data-slot="banner"] [data-slot="pill"]',
      '[data-slot="step-ladder"] li',
      '[data-slot="shared-sheet"]',
    ]) {
      expect(marked(selector), selector).toBeInTheDocument()
    }
  })

  it('draw the bar’s rule and the strip’s edge from the live token, and none names a phase', () => {
    renderTable({ seats: 2 })
    expect(marked('[data-region="bar"]')).toHaveClass('border-b-[3px]', 'border-phase')
    expect(marked('[data-slot="shared-sheet"]')).toHaveClass('border-2', 'border-phase')
    for (const part of document.querySelectorAll<HTMLElement>('[data-themed]')) {
      expect(part.getAttribute('class') ?? '', part.dataset.slot).not.toMatch(
        /phase-(beginning|main|combat|ending)/,
      )
    }
  })

  it('all lie inside the table root, where the stylesheet gives them a theme', () => {
    renderTable({ seats: 2 })
    for (const part of document.querySelectorAll<HTMLElement>('[data-themed]')) {
      expect(root()).toContainElement(part)
    }
  })

  it('keep the strip’s gold glow on your own turn, outside the edge of the phase', () => {
    renderTable({ fixture: buildFixture({ seats: 2, active: 0, permanents: 2 }) })
    expect(marked('[data-slot="shared-sheet"]')).toHaveClass('your-turn-cue', 'border-phase')
  })
})
