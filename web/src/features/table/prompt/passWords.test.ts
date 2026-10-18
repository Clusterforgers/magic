// Goal: the primary button says what passing does now, in plain words and never "pass priority": over a spell it resolves it, on the viewer's turn it names the next step (the last window ends the turn), on anybody else's it continues; the table covers every step and both turn owners, and the words of the yields and of the situation read off the table are fixed too.
import { describe, expect, it } from 'vitest'
import { initialPlayState } from '@/play/store'
import type { PlayState } from '@/play/store'
import type { Step, YieldUntil } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { buildFixture } from '../dev/fixture'
import { fixtureState } from '../dev/devGame'
import { STEP_LABEL } from '../turnText'
import { passSituation, primaryDescription, primaryEndsTurn, primaryLabel, YIELD_WORDS } from './passWords'
import type { PassSituation } from './passWords'

const STEPS = Object.keys(STEP_LABEL) as Step[]

/** What the primary button says on the viewer's own turn with an empty stack, by the step it is pressed in. */
const OWN_TURN: Record<Step, string> = {
  Untap: 'Next: Upkeep',
  Upkeep: 'Next: Draw',
  Draw: 'Next: Precombat main',
  Main1: 'Next: Combat',
  BeginCombat: 'Next: Declare attackers',
  DeclareAttackers: 'Next: Declare blockers',
  DeclareBlockers: 'Next: Combat damage',
  FirstStrikeDamage: 'Next: Combat damage',
  CombatDamage: 'Next: End of combat',
  EndCombat: 'Next: Postcombat main',
  Main2: 'Next: End step',
  End: 'End turn',
  Cleanup: 'End turn',
}

const situation = (step: Step, yourTurn: boolean, top: string | null = null): PassSituation => ({
  step,
  yourTurn,
  top,
})

describe('the primary button', () => {
  it('has a row for every step of the turn', () => {
    expect(Object.keys(OWN_TURN).sort()).toEqual([...STEPS].sort())
    expect(STEPS).toHaveLength(13)
  })

  it.each(STEPS)('on your own turn in %s with an empty stack it names what comes next', (step) => {
    expect(primaryLabel(situation(step, true))).toBe(OWN_TURN[step])
  })

  it.each(STEPS)('on anybody else’s turn in %s with an empty stack it says Continue', (step) => {
    expect(primaryLabel(situation(step, false))).toBe('Continue')
  })

  it.each(STEPS.flatMap((step) => [true, false].map((yourTurn): [Step, boolean] => [step, yourTurn])))(
    'over a spell on the stack in %s (your turn: %s) it resolves the top one by name',
    (step, yourTurn) => {
      expect(primaryLabel(situation(step, yourTurn, 'Lightning Bolt'))).toBe('Resolve Lightning Bolt')
    },
  )

  it('uses the name as it is, however long', () => {
    const name = "Sword of Fire and Ice's Sibling, the Very Long-Named Enchantment"
    expect(primaryLabel(situation('Main2', true, name))).toBe(`Resolve ${name}`)
  })

  it.each(STEPS.flatMap((step) => [true, false].map((yourTurn): [Step, boolean] => [step, yourTurn])))(
    'in %s (your turn: %s) never says "pass priority", with or without a spell',
    (step, yourTurn) => {
      for (const top of [null, 'Giant Growth']) {
        const label = primaryLabel(situation(step, yourTurn, top))
        expect(label).not.toMatch(/pass/i)
        expect(label.trim()).not.toBe('')
      }
    },
  )

  it('is the only one that ends the turn: the last window of your own turn', () => {
    const ends = STEPS.flatMap((step) =>
      [true, false].flatMap((yourTurn) =>
        [null, 'Bolt'].map((top): [Step, boolean, string | null] => [step, yourTurn, top]),
      ),
    ).filter(([step, yourTurn, top]) => primaryEndsTurn(situation(step, yourTurn, top)))
    expect(ends).toEqual([
      ['End', true, null],
      ['Cleanup', true, null],
    ])
  })
})

describe('what a screen reader is told', () => {
  it('says the button passes priority, and what then happens to a spell', () => {
    expect(primaryDescription(situation('Main1', true))).toBe('Passes priority.')
    expect(primaryDescription(situation('Main1', false, 'Lightning Bolt'))).toBe(
      'Passes priority; if nobody responds, Lightning Bolt resolves.',
    )
  })

  it.each(Object.entries(YIELD_WORDS) as [YieldUntil, (typeof YIELD_WORDS)[YieldUntil]][])(
    '%s has an action, a line for the table and a description',
    (_until, words) => {
      expect(words.action).not.toMatch(/priority/i)
      expect(words.passing).toMatch(/^Passing until /)
      expect(words.description).toMatch(/^Passes priority/)
      expect(words.description).toMatch(/asked again if an opponent adds to/)
    },
  )

  it('words the three yields as the table, the menu and the indicator show them', () => {
    expect(YIELD_WORDS.Resolve.action).toBe('Resolve everything on the stack')
    expect(YIELD_WORDS.EndOfTurn.action).toBe('End turn')
    expect(YIELD_WORDS.MyTurn.action).toBe('Pass until my next turn')
    expect(YIELD_WORDS.EndOfTurn.passing).toBe('Passing until the end of turn')
    expect(YIELD_WORDS.Resolve.passing).toBe('Passing until the stack is empty')
    expect(YIELD_WORDS.MyTurn.passing).toBe('Passing until your next turn')
  })
})

describe('what the controls read off the table', () => {
  const fixture = buildFixture({ seats: 2 })
  const state = (): PlayState => fixtureState(fixture)

  it('reads the step, whose turn it is and the top of the stack', () => {
    const read = passSituation(state())
    expect(read.step).toBe(fixture.view.turn.step)
    expect(read.yourTurn).toBe(fixture.view.turn.active === fixture.view.viewer)
    const topId = must(fixture.view.stack.cards?.at(-1))
    expect(read.top).toBe(must(fixture.view.objects.find((object) => object.id === topId)).name)
  })

  it('knows when it is the viewer’s turn', () => {
    const own = buildFixture({ seats: 2, active: 0 })
    expect(passSituation(fixtureState(own)).yourTurn).toBe(true)
    const other = buildFixture({ seats: 2, active: 1 })
    expect(passSituation(fixtureState(other)).yourTurn).toBe(false)
  })

  it('has no top when the stack is empty', () => {
    const next = state()
    const emptied = clone(must(next.view))
    emptied.stack = { count: 0, cards: [] }
    expect(passSituation({ ...next, view: emptied }).top).toBeNull()
  })

  it('has no top for a stack object it cannot name, and falls back to the first step before there is a table', () => {
    const next = state()
    const strange = clone(must(next.view))
    strange.stack = { count: 1, cards: [999_999] }
    expect(passSituation({ ...next, view: strange }).top).toBeNull()
    expect(passSituation(initialPlayState())).toEqual({ step: 'Main1', yourTurn: false, top: null })
  })
})
