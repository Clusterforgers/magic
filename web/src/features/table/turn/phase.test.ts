// Goal: every step the engine has belongs to exactly one of four phase themes (beginning, main, combat, ending) by a pure rule on the step alone, the turn visits them in order with the main phase on both sides of combat, and each phase has a name and an icon of its own, so the theme is never told by its colour alone.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import type { Step } from '@/protocol/generated'
import { STEP_LABEL } from '../turnText'
import { PHASES, phaseOf } from './phase'
import type { PhaseId } from './phase'

const STEPS = Object.keys(STEP_LABEL) as Step[]

const EXPECTED: ReadonlyArray<[Step, PhaseId]> = [
  ['Untap', 'beginning'],
  ['Upkeep', 'beginning'],
  ['Draw', 'beginning'],
  ['Main1', 'main'],
  ['BeginCombat', 'combat'],
  ['DeclareAttackers', 'combat'],
  ['DeclareBlockers', 'combat'],
  ['FirstStrikeDamage', 'combat'],
  ['CombatDamage', 'combat'],
  ['EndCombat', 'combat'],
  ['Main2', 'main'],
  ['End', 'ending'],
  ['Cleanup', 'ending'],
]

describe('phaseOf', () => {
  it.each(EXPECTED)('puts %s in the %s phase', (step, id) => {
    expect(phaseOf(step).id).toBe(id)
  })

  it('knows every step the views carry, which is the protocol’s list of steps', () => {
    const generated = readFileSync(join(import.meta.dirname, '../../../protocol/generated/Step.ts'), 'utf8')
    const protocol = [...generated.matchAll(/"(\w+)"/g)].map((match) => match[1])
    expect(protocol).toHaveLength(13)
    expect(STEPS).toEqual(protocol)
    expect(EXPECTED.map(([step]) => step)).toEqual(protocol)
    for (const step of protocol) expect(phaseOf(step as Step), step).toBeDefined()
  })

  it('walks the turn in order: beginning, main, combat, main, ending', () => {
    const walk: PhaseId[] = []
    for (const step of STEPS) {
      const { id } = phaseOf(step)
      if (walk.at(-1) !== id) walk.push(id)
    }
    expect(walk).toEqual(['beginning', 'main', 'combat', 'main', 'ending'])
  })

  it('returns the same phase object for every step of it, so an equal id means an equal theme', () => {
    for (const step of STEPS) expect(phaseOf(step)).toBe(PHASES[phaseOf(step).id])
  })

  it('reads the step and nothing else: a pure function of one value', () => {
    expect(phaseOf.length).toBe(1)
    for (const step of STEPS) expect(phaseOf(step)).toBe(phaseOf(step))
  })
})

describe('the four phases', () => {
  const phases = Object.values(PHASES)

  it('are beginning, main, combat and ending', () => {
    expect(Object.keys(PHASES)).toEqual(['beginning', 'main', 'combat', 'ending'])
    for (const [id, phase] of Object.entries(PHASES)) expect(phase.id).toBe(id)
  })

  it('each have a name, spoken as a word, and no two share one', () => {
    for (const phase of phases) expect(phase.name).toMatch(/^[A-Z][a-z]+$/)
    expect(new Set(phases.map((phase) => phase.name)).size).toBe(4)
  })

  it('each have an icon of their own, so that colour is never the only sign', () => {
    for (const phase of phases) expect(phase.icon, phase.id).toBeDefined()
    expect(new Set(phases.map((phase) => phase.icon)).size).toBe(4)
  })

  it('use every phase for at least one step, and no more than four themes exist', () => {
    const used = new Set(STEPS.map((step) => phaseOf(step).id))
    expect([...used].sort()).toEqual(Object.keys(PHASES).sort())
  })
})
