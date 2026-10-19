// Goal: the ladder marks every step of a turn as passed, current, still to come or (for a moment) skipped from where the turn stands alone, whatever the steps the ladder draws and wherever the turn is, including before the first window and after the last.
import { describe, expect, it } from 'vitest'
import type { Step } from '@/protocol/generated'
import { pipStates } from './ladder'
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
