// Goal: a reworked boss's words are right at every point of the fight: the countdown to Phase 1 ("in N rounds", "at the end of this round"), the phase in force with the countdown to the next, no countdown once both began; a reusable Satchel card's "returns · +{N}" against a once card's "once"; the abilities list innate (with its level), modifier, relics (copies counted) and phases with their rounds; an old boss is not reworked.
import { describe, expect, it } from 'vitest'
import type { BossView, RoundPhaseView } from '@/protocol/generated'
import { devReworkedView } from '../dev/reworkedBoss'
import { isReworked, phaseCountdown, reworkedAbilities, satchelMark, satchelMarkTitle } from './reworkedBoss'

const phase = (name: string, round: number, begun: boolean): RoundPhaseView => ({
  name,
  text: `${name} rules.`,
  round,
  begun,
})

const view = (begun: [boolean, boolean], phaseIn?: number): BossView => ({
  phases: ['Stir', 'Tide'],
  begun: begun.filter(Boolean).length,
  round_phases: [phase('Stir', 8, begun[0]), phase('Tide', 14, begun[1])],
  ...(phaseIn === undefined ? {} : { phase_in: phaseIn }),
})

describe('phaseCountdown', () => {
  it.each([
    [view([false, false], 8), 'Phase 1 in 8 rounds', false, undefined],
    [view([false, false], 2), 'Phase 1 in 2 rounds', false, undefined],
    [view([false, false], 1), 'Phase 1 at the end of this round', false, undefined],
    [view([true, false], 6), 'Stir', true, 'Phase 2 in 6 rounds'],
    [view([true, false], 1), 'Stir', true, 'Phase 2 at the end of this round'],
    [view([true, true]), 'Tide', true, undefined],
  ] as const)('%#: says "%s"', (boss, label, begun, next) => {
    const countdown = phaseCountdown(boss)
    expect(countdown?.label).toBe(label)
    expect(countdown?.begun).toBe(begun)
    expect(countdown?.next).toBe(next)
    expect(countdown?.title).toMatch(/\.$/)
  })

  it('is none for a boss without phases by round', () => {
    expect(phaseCountdown({ phases: ['Old'], begun: 0 })).toBeNull()
    expect(isReworked({})).toBe(false)
    expect(isReworked(view([false, false], 3))).toBe(true)
  })
})

describe('satchelMark', () => {
  it.each([
    [true, 0, 'returns · +{0}', /next cast costs \{0\} more/],
    [true, 4, 'returns · +{4}', /next cast costs \{4\} more/],
    [false, 0, 'once', /once this fight/],
  ] as const)('reusable %s with tax %i reads "%s"', (reusable, tax, mark, title) => {
    const card = { name: 'Rat Out', reusable, tax }
    expect(satchelMark(card)).toBe(mark)
    expect(satchelMarkTitle(card)).toMatch(title)
  })
})

describe('reworkedAbilities', () => {
  it.each([1, 2] as const)('lists the dev boss at stage %i in reading order', (stage) => {
    const abilities = reworkedAbilities(devReworkedView(stage, 900))
    expect(abilities.map((ability) => [ability.kind, ability.name, ability.when])).toEqual([
      ['innate', 'Brood III', 'Level III'],
      ['modifier', 'Deep Pockets', undefined],
      ['relic', 'Whetstone ×2', undefined],
      stage === 2
        ? ['phase', 'The Nest Stirs', 'Phase 1']
        : ['next-phase', 'The Nest Stirs', 'Phase 1 at round 8'],
      ['next-phase', 'Plague Tide', 'Phase 2 at round 14'],
    ])
    for (const ability of abilities) expect(ability.text).not.toBe('')
  })
})
