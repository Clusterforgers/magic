// Goal: the ladder marks every step of a turn as passed, current, still to come or (for a moment) skipped from where the turn stands alone, whatever the steps the ladder draws and wherever the turn is, including before the first window and after the last, and cuts its steps into the runs of one phase that carry the phase's icon and name.
import { describe, expect, it } from 'vitest'
import type { Step } from '@/protocol/generated'
import { phaseRuns, pipStates } from './ladder'
import type { PipState } from './ladder'

const STEPS: Step[] = [
  'Upkeep',
  'Draw',
  'Main1',
  'BeginCombat',
  'DeclareAttackers',
  'DeclareBlockers',
  'CombatDamage',
  'EndCombat',
  'Main2',
  'End',
]

const states = (current: Step, dashed: Step[] = [], steps: Step[] = STEPS): string =>
  pipStates(steps, current, dashed)
    .map((state) => ({ upcoming: '.', passed: '#', current: '*', skipped: '-' })[state])
    .join('')

describe('the marks of a turn', () => {
  it.each<[Step, string]>([
    ['Untap', '..........'],
    ['Upkeep', '*.........'],
    ['Draw', '#*........'],
    ['Main1', '##*.......'],
    ['DeclareBlockers', '#####*....'],
    ['CombatDamage', '######*...'],
    ['Main2', '########*.'],
    ['End', '#########*'],
    ['Cleanup', '##########'],
  ])('with the turn in %s it reads %s', (current, expected) => {
    expect(states(current)).toBe(expected)
  })

  it('dashes a passed step that was skipped, and only a passed one', () => {
    expect(states('Main1', ['Upkeep', 'Draw'])).toBe('--*.......')
    expect(states('Main1', ['Upkeep', 'End'])).toBe('-#*.......')
    expect(states('Draw', ['Draw', 'Main2'])).toBe('#*........')
  })

  it('places first-strike damage between blockers and regular damage when it is drawn', () => {
    const withStrike: Step[] = [...STEPS.slice(0, 6), 'FirstStrikeDamage', ...STEPS.slice(6)]
    expect(states('FirstStrikeDamage', [], withStrike)).toBe('######*....')
    expect(states('CombatDamage', [], withStrike)).toBe('#######*...')
    // A game that skips it stays right when the step is not drawn but is the current one.
    expect(states('FirstStrikeDamage')).toBe('######....')
  })

  describe('the runs of one phase', () => {
    const runs = (steps: Step[]): string =>
      phaseRuns(steps)
        .map((run) => `${run.phase.id}:${run.steps.join('+')}`)
        .join(' ')

    it('cuts the ladder into five runs, the main phase coming twice around combat', () => {
      expect(runs(STEPS)).toBe(
        'beginning:Upkeep+Draw main:Main1 combat:BeginCombat+DeclareAttackers+DeclareBlockers+CombatDamage+EndCombat main:Main2 ending:End',
      )
    })

    it('keeps first-strike damage in the combat run', () => {
      const withStrike: Step[] = [...STEPS.slice(0, 6), 'FirstStrikeDamage', ...STEPS.slice(6)]
      expect(phaseRuns(withStrike)[2]?.steps).toContain('FirstStrikeDamage')
      expect(phaseRuns(withStrike)).toHaveLength(5)
    })

    it.each<[Step[], string]>([
      [[], ''],
      [['Main1', 'Main2'], 'main:Main1+Main2'],
      [['Draw', 'Main2', 'End'], 'beginning:Draw main:Main2 ending:End'],
    ])('cuts %j into %j', (steps, expected) => {
      expect(runs(steps)).toBe(expected)
    })

    it('loses and repeats no step', () => {
      expect(phaseRuns(STEPS).flatMap((run) => run.steps)).toEqual(STEPS)
    })
  })

  it('is the same for any ladder: one lit step at most, passed before it and to come after it', () => {
    for (const current of [...STEPS, 'Untap', 'Cleanup'] as Step[]) {
      const found = pipStates(STEPS, current, []) as PipState[]
      expect(found.filter((state) => state === 'current').length).toBeLessThanOrEqual(1)
      const lit = found.indexOf('current')
      if (lit >= 0) {
        expect(found.slice(0, lit).every((state) => state === 'passed')).toBe(true)
        expect(found.slice(lit + 1).every((state) => state === 'upcoming')).toBe(true)
      }
    }
  })
})
