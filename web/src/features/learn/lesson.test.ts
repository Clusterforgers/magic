// Goal: the lesson step machine moves on exactly when the game shows a step done: a read step waits for Next, an action step for its fact, several steps done at once are passed together, and the guided game's facts in the order the tutorial table plays them walk every step to the end.
import { describe, expect, it } from 'vitest'
import { advance, BOT_LIFE, GAME_LESSON, MANUAL_LESSON } from './lesson'
import type { LessonFacts, LessonStep, Named } from './lesson'

const none: LessonFacts = {
  turn: 1,
  mine: true,
  step: 'Main1',
  hand: [],
  lands: [],
  creatures: [],
  theirCreatures: [],
  command: [],
  stack: [],
  attacking: 0,
  blocking: 0,
  attacked: false,
  theirLife: BOT_LIFE,
  byHand: 0,
  over: false,
  won: false,
}

let next = 100
const named = (...names: string[]): Named[] => names.map((name) => ({ name, id: (next += 1) }))

/** The facts at each moment of the lesson, as the tutorial table plays it. */
const START: LessonFacts = {
  ...none,
  hand: named('Forest', 'Plains', 'Grizzly Bears', 'Hero in Training'),
  lands: named('Forest', 'Forest'),
  command: named('Isamaru, Hound of Konda'),
}
const LANDED: LessonFacts = { ...START, lands: named('Forest', 'Forest', 'Forest') }
const BEARS: LessonFacts = { ...LANDED, creatures: named('Grizzly Bears') }
const BOT_TURN: LessonFacts = { ...BEARS, turn: 2, mine: false }
const THIRD: LessonFacts = { ...BOT_TURN, turn: 3, mine: true, theirCreatures: named('Goblin Piker') }
const PLAINS: LessonFacts = { ...THIRD, lands: named('Forest', 'Forest', 'Forest', 'Plains') }
const ATTACK: LessonFacts = { ...PLAINS, attacking: 1, step: 'DeclareAttackers' }
const HERO: LessonFacts = {
  ...ATTACK,
  attacking: 0,
  theirLife: BOT_LIFE - 2,
  step: 'Main2',
  stack: [{ ...named('Hero in Training')[0]!, mine: true }],
}
const DRAWN: LessonFacts = {
  ...HERO,
  stack: [],
  creatures: named('Grizzly Bears', 'Hero in Training'),
  hand: named('Giant Growth'),
}
const FOURTH: LessonFacts = { ...DRAWN, turn: 4, mine: false, attacked: true }
const BLOCK: LessonFacts = { ...FOURTH, blocking: 1 }
const GROWTH: LessonFacts = { ...BLOCK, hand: [] }
const FIFTH: LessonFacts = { ...GROWTH, turn: 5, mine: true, blocking: 0, theirCreatures: [] }
const ISAMARU: LessonFacts = { ...FIFTH, command: [] }
const WON: LessonFacts = { ...ISAMARU, over: true, won: true }

const ids = (steps: readonly LessonStep[]): string[] => steps.map((step) => step.id)

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
    ['an action step waits for its fact', [land, cast], 0, START, 0],
    ['a done step moves on', [land, cast], 0, LANDED, 1],
    ['several steps done at once are passed together', [land, cast, read], 0, BEARS, 2],
    ['a read step stops the walk', [land, read, cast], 0, BEARS, 1],
    ['past the last step is the length', [land, cast], 0, BEARS, 2],
  ] as const)('%s', (_, steps, from, facts, expected) => {
    expect(advance(steps, from, facts)).toBe(expected)
  })
})

describe('the guided game', () => {
  it('teaches the owner’s list of moments in order', () => {
    expect(ids(GAME_LESSON.steps)).toEqual([
      'welcome',
      'land',
      'cast',
      'pass',
      'their-turn',
      'second-land',
      'attack',
      'hero',
      'trigger',
      'end-turn',
      'block',
      'instant',
      'resolve',
      'commander',
      'win',
      'won',
    ])
  })

  it.each([
    ['land', START, LANDED],
    ['cast', LANDED, BEARS],
    ['pass', BEARS, BOT_TURN],
    ['their-turn', BOT_TURN, THIRD],
    ['second-land', THIRD, PLAINS],
    ['attack', PLAINS, ATTACK],
    ['hero', ATTACK, HERO],
    ['trigger', HERO, DRAWN],
    ['end-turn', DRAWN, FOURTH],
    ['block', FOURTH, BLOCK],
    ['instant', BLOCK, GROWTH],
    ['resolve', GROWTH, FIFTH],
    ['commander', FIFTH, ISAMARU],
    ['win', ISAMARU, WON],
  ] as const)('the %s step waits before the move and is done after it', (id, before, after) => {
    const index = GAME_LESSON.steps.findIndex((step) => step.id === id)
    expect(advance(GAME_LESSON.steps, index, before)).toBe(index)
    expect(advance(GAME_LESSON.steps, index, after)).toBeGreaterThan(index)
  })

  it('walks from the first land to the last read step on the table’s facts', () => {
    let at = advance(GAME_LESSON.steps, 1, START)
    for (const facts of [
      LANDED,
      BEARS,
      BOT_TURN,
      THIRD,
      PLAINS,
      ATTACK,
      HERO,
      DRAWN,
      FOURTH,
      BLOCK,
      GROWTH,
      FIFTH,
      ISAMARU,
      WON,
    ])
      at = advance(GAME_LESSON.steps, at, facts)
    expect(GAME_LESSON.steps[at]?.id).toBe('won')
  })

  it.each([
    ['block', FOURTH],
    ['instant', BLOCK],
    ['resolve', GROWTH],
    ['commander', FIFTH],
  ] as const)('lands on the %s step for a player who comes back in the middle of the game', (id, facts) => {
    expect(GAME_LESSON.steps[advance(GAME_LESSON.steps, 1, facts)]?.id).toBe(id)
  })

  it('points each action step at the card it names while the card is in view', () => {
    const land = GAME_LESSON.steps.find((step) => step.id === 'land')
    const forest = START.hand[0]!
    expect(land?.targets(START)[0]).toBe(`[data-object-id="${forest.id}"]`)
    const cast = GAME_LESSON.steps.find((step) => step.id === 'cast')
    expect(cast?.targets({ ...START, hand: [] })).not.toContain(`[data-object-id="${forest.id}"]`)
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
