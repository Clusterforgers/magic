// Goal: the lesson step machine moves on exactly when the game shows a step done: a read step waits for Next, an action step for its fact, several steps done at once are passed together; and on the decisions the tutorial table really sends (`tutorialTrace.json`, written by the server's tutorial test), every decision of the player meets a step shown at that kind of decision that names the button or card to press.
import { describe, expect, it } from 'vitest'
import { primaryLabel } from '@/features/table/prompt/passWords'
import { advance, GAME_LESSON, MANUAL_LESSON, PASS, stepText } from './lesson'
import type { LessonFacts, LessonStep } from './lesson'
import trace from './tutorialTrace.json'

/** One moment of the trace: the facts at a decision of the player's, and what the script answers. */
interface Moment {
  act: 'Land' | 'Cast' | 'Pass' | 'PayAuto' | 'Attack' | 'Block' | 'Target' | null
  cards: string[]
  facts: LessonFacts
}

const MOMENTS = trace as unknown as Moment[]

const none: LessonFacts = { ...MOMENTS[0]!.facts, decision: null }
const LANDED: LessonFacts = { ...none, lands: [...none.lands, { name: 'Forest', id: 1 }] }
const BEARS: LessonFacts = { ...LANDED, creatures: [{ name: 'Grizzly Bears', id: 2 }] }

/** The words of the button that does `moment`'s action, and the cards it names. */
function wordsOf(moment: Moment): string[] {
  const { facts } = moment
  switch (moment.act) {
    case 'Pass': {
      const top = facts.stack.at(-1)?.name ?? null
      const label = primaryLabel({ step: facts.step!, yourTurn: facts.mine, top })
      return [label ?? 'End turn']
    }
    case 'PayAuto':
      return ['Pay automatically']
    case 'Attack':
      return ['press Attack', ...moment.cards]
    case 'Block':
      return ['press Block', ...moment.cards]
    case 'Target':
      return ['Confirm targets', ...moment.cards]
    default:
      return moment.cards.map((name) => name.split(',')[0]!)
  }
}

describe('advance', () => {
  const read: LessonStep = { id: 'read', title: '', text: '', targets: () => [] }
  const land: LessonStep = {
    id: 'land',
    title: '',
    text: '',
    targets: () => [],
    done: (f) => f.lands.length > 2,
  }
  const cast: LessonStep = {
    id: 'cast',
    title: '',
    text: '',
    targets: () => [],
    done: (f) => f.creatures.length > 0,
  }

  it.each([
    ['a read step waits for Next', [read, land], 0, BEARS, 0],
    ['an action step waits for its fact', [land, cast], 0, none, 0],
    ['a done step moves on', [land, cast], 0, LANDED, 1],
    ['several steps done at once are passed together', [land, cast, read], 0, BEARS, 2],
    ['a read step stops the walk', [land, read, cast], 0, BEARS, 1],
    ['past the last step is the length', [land, cast], 0, BEARS, 2],
  ] as const)('%s', (_, steps, from, facts, expected) => {
    expect(advance(steps, from, facts)).toBe(expected)
  })
})

describe('the guided game on the table’s decisions', () => {
  it('shows, at each decision of the player, a step for that decision that names what to press', () => {
    let at = 1
    for (const moment of MOMENTS.slice(0, -1)) {
      at = advance(GAME_LESSON.steps, at, moment.facts)
      const step = GAME_LESSON.steps[at]
      const where = `turn ${moment.facts.turn} ${moment.facts.step} ${moment.facts.decision} (${step?.id})`
      expect(step?.decisions, where).toContain(moment.facts.decision)
      const text = step === undefined ? '' : stepText(step, moment.facts)
      for (const words of wordsOf(moment)) expect(text, where).toContain(words)
    }
    at = advance(GAME_LESSON.steps, at, MOMENTS.at(-1)!.facts)
    expect(GAME_LESSON.steps[at]?.id).toBe('won')
  })

  it('names the primary button with the words the dock draws', () => {
    const step = { text: `Press ${PASS}.` }
    const declared = MOMENTS.find((each) => each.facts.step === 'DeclareAttackers' && each.act === 'Pass')!
    expect(stepText(step, declared.facts)).toBe('Press Next: Declare blockers.')
    expect(stepText(step, declared.facts, 'Next: Blockers')).toBe('Press Next: Blockers.')
    expect(stepText({ text: 'No button.' }, declared.facts)).toBe('No button.')
  })

  it('uses every step that waits on the player, in order', () => {
    const shown: string[] = []
    let at = 1
    for (const moment of MOMENTS) {
      at = advance(GAME_LESSON.steps, at, moment.facts)
      const id = GAME_LESSON.steps[at]?.id
      if (id !== undefined && shown.at(-1) !== id) shown.push(id)
    }
    const waiting = GAME_LESSON.steps
      .filter((step) => (step.decisions?.length ?? 0) > 0)
      .map((step) => step.id)
    expect(shown.filter((id) => waiting.includes(id))).toEqual(waiting)
  })

  it('names only cards the table deals', () => {
    const names = new Set(
      MOMENTS.flatMap(({ facts }) =>
        [...facts.hand, ...facts.lands, ...facts.creatures, ...facts.theirCreatures, ...facts.command].map(
          (each) => each.name,
        ),
      ),
    )
    for (const name of [
      'Grizzly Bears',
      'Hero in Training',
      'Giant Growth',
      'Dregscape Zombie',
      'Giada, Font of Hope',
    ])
      expect(names).toContain(name)
    const text = GAME_LESSON.steps.map((step) => step.text).join(' ')
    expect(text).not.toMatch(/Isamaru|Goblin Piker/)
  })

  it.each([
    ['block', 'DeclareBlockers'],
    ['instant', 'Priority'],
    ['third-land', 'Priority'],
  ] as const)('lands on the %s step for a player who comes back at that decision', (id, decision) => {
    const moment = MOMENTS.find((each) => {
      const at = advance(GAME_LESSON.steps, 1, each.facts)
      return GAME_LESSON.steps[at]?.id === id
    })
    expect(moment?.facts.decision).toBe(decision)
  })

  it('points each action step at the card it names while the card is in view', () => {
    const land = GAME_LESSON.steps.find((step) => step.id === 'land')
    const forest = none.hand.find((each) => each.name === 'Forest')!
    expect(land?.targets(none)[0]).toBe(`[data-object-id="${forest.id}"]`)
    const cast = GAME_LESSON.steps.find((step) => step.id === 'cast')
    expect(cast?.targets({ ...none, hand: [] })).not.toContain(`[data-object-id="${forest.id}"]`)
  })
})

describe('the manual lesson', () => {
  it('moves on for each action taken by hand', () => {
    const move = MANUAL_LESSON.steps.findIndex((step) => step.id === 'move')
    expect(advance(MANUAL_LESSON.steps, move, none)).toBe(move)
    expect(MANUAL_LESSON.steps[advance(MANUAL_LESSON.steps, move, { ...none, byHand: 1 })]?.id).toBe('menu')
    expect(MANUAL_LESSON.steps[advance(MANUAL_LESSON.steps, move, { ...none, byHand: 2 })]?.id).toBe('done')
  })
})
