// Goal: the ladder draws the ten steps that give priority in turn order (first-strike damage only while it is current or holds a stop), reads how a stop is held from the server's list, asks for the right SetStop for every state and kind of press, and says every step, side and state in words, starting with "Your upkeep stop: off. Press to stop here this turn".
import { describe, expect, it } from 'vitest'
import { STOP_STEPS } from '@/play/stops'
import type { StopStep } from '@/play/stops'
import type { Step, Stop, StopSide } from '@/protocol/generated'
import {
  ladderSteps,
  pressRequest,
  rowName,
  stepWords,
  stopCount,
  stopDescription,
  stopName,
  stopState,
  turnSide,
} from './stopModel'
import type { StopState } from './stopModel'

const SIDES: StopSide[] = ['Own', 'Opponent']
const STATES: StopState[] = ['off', 'turn', 'lasting']
const TEN: StopStep[] = [
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
const ALL_STEPS: Step[] = ['Untap', ...STOP_STEPS, 'Cleanup']

const stop = (side: StopSide, step: Step, keep = false): Stop => ({ side, step, keep })

describe('the steps the ladder draws', () => {
  it.each(ALL_STEPS)('are the ten in turn order when the step is %s and no stop is held', (current) => {
    const expected = current === 'FirstStrikeDamage' ? STOP_STEPS : TEN
    expect(ladderSteps(current, [])).toEqual(expected)
  })

  it('are the ten outside the turns, where there is no current step', () => {
    expect(ladderSteps(null, [])).toEqual(TEN)
  })

  it('add first-strike damage, between declare blockers and combat damage, when the step is current', () => {
    const steps = ladderSteps('FirstStrikeDamage', [])
    expect(steps).toHaveLength(11)
    expect(steps.indexOf('FirstStrikeDamage')).toBe(steps.indexOf('DeclareBlockers') + 1)
    expect(steps.indexOf('CombatDamage')).toBe(steps.indexOf('FirstStrikeDamage') + 1)
  })

  it.each(SIDES)(
    'keep first-strike damage while a stop is held there on the %s side, so it can be seen and cleared',
    (side) => {
      expect(ladderSteps('Main1', [stop(side, 'FirstStrikeDamage')])).toEqual(STOP_STEPS)
    },
  )

  it('draw no untap and no cleanup, which cannot hold a stop', () => {
    for (const current of ALL_STEPS) {
      expect(ladderSteps(current, [])).not.toContain('Untap')
      expect(ladderSteps(current, [])).not.toContain('Cleanup')
    }
  })

  it('stay the same list while nothing about them changes, so a row does not redraw for it', () => {
    expect(ladderSteps('Main1', [])).toBe(ladderSteps('End', [stop('Own', 'Draw')]))
    expect(ladderSteps('FirstStrikeDamage', [])).toBe(
      ladderSteps('Main2', [stop('Own', 'FirstStrikeDamage')]),
    )
  })
})

describe('how a stop is held', () => {
  it.each([
    ['nothing held', [], 'Own', 'Main1', 'off'],
    ['a one-turn stop', [stop('Own', 'Main1')], 'Own', 'Main1', 'turn'],
    ['a lasting stop', [stop('Own', 'Main1', true)], 'Own', 'Main1', 'lasting'],
    ['the other side held', [stop('Opponent', 'Main1', true)], 'Own', 'Main1', 'off'],
    ['the other step held', [stop('Own', 'Main2', true)], 'Own', 'Main1', 'off'],
    [
      'both sides, each its own',
      [stop('Own', 'End'), stop('Opponent', 'End', true)],
      'Opponent',
      'End',
      'lasting',
    ],
    ['both sides, each its own', [stop('Own', 'End'), stop('Opponent', 'End', true)], 'Own', 'End', 'turn'],
  ] satisfies [string, Stop[], StopSide, Step, StopState][])(
    'reads %s at %s %s as %s',
    (_what, stops, side, step, state) => {
      expect(stopState(stops, side, step)).toBe(state)
    },
  )

  it.each([
    [0, 0, 'Own'],
    [1, 0, 'Opponent'],
    [3, 3, 'Own'],
    [0, 5, 'Opponent'],
  ] satisfies [number, number, StopSide][])(
    'calls the turn of seat %i, seen from seat %i, %s',
    (active, seat, side) => {
      expect(turnSide(active, seat)).toBe(side)
    },
  )
})

describe('what a press asks for', () => {
  it.each([
    // [state, the press stands for a lasting stop, request]
    ['off', false, { on: true, keep: false }],
    ['off', true, { on: true, keep: true }],
    ['turn', false, { on: false, keep: false }],
    ['turn', true, { on: true, keep: true }],
    ['lasting', false, { on: false, keep: true }],
    ['lasting', true, { on: false, keep: true }],
  ] satisfies [StopState, boolean, { on: boolean; keep: boolean }][])(
    'in state %s, lasting %s: %j',
    (state, lasting, request) => {
      expect(pressRequest(state, lasting)).toEqual(request)
    },
  )

  it('never sets a stop that is already held with the keep it has', () => {
    for (const state of STATES) {
      for (const lasting of [true, false]) {
        const request = pressRequest(state, lasting)
        expect(request.on && state === (request.keep ? 'lasting' : 'turn')).toBe(false)
      }
    }
  })
})

describe('the words of a step', () => {
  it.each(STOP_STEPS)('%s has words, different from every other step', (step) => {
    expect(stepWords(step)).not.toBe('')
    expect(stepWords(step)).toBe(stepWords(step).toLowerCase())
    expect(new Set(STOP_STEPS.map(stepWords)).size).toBe(STOP_STEPS.length)
  })
})

describe('the spoken text of a button', () => {
  it('is, for your upkeep with nothing set, the sentence the player was promised', () => {
    expect(stopName('Own', 'Upkeep', 'off', false)).toBe(
      'Your upkeep stop: off. Press to stop here this turn',
    )
  })

  it.each([
    ['Own', 'Upkeep', 'off', false, 'Your upkeep stop: off. Press to stop here this turn'],
    ['Own', 'Main1', 'off', true, 'Your precombat main stop: off. Press to stop here every turn'],
    [
      'Own',
      'BeginCombat',
      'turn',
      false,
      'Your beginning of combat stop: on for one turn. Press to turn it off',
    ],
    [
      'Own',
      'DeclareAttackers',
      'turn',
      true,
      'Your declare attackers stop: on for one turn. Press to keep it every turn',
    ],
    [
      'Own',
      'DeclareBlockers',
      'lasting',
      false,
      'Your declare blockers stop: on every turn. Press to turn it off',
    ],
    [
      'Own',
      'FirstStrikeDamage',
      'lasting',
      true,
      'Your first-strike damage stop: on every turn. Press to turn it off',
    ],
    [
      'Opponent',
      'CombatDamage',
      'off',
      false,
      "Opponents' combat damage stop: off. Press to stop here on one opponent turn",
    ],
    [
      'Opponent',
      'EndCombat',
      'off',
      true,
      "Opponents' end of combat stop: off. Press to stop here on every opponent turn",
    ],
    [
      'Opponent',
      'Main2',
      'turn',
      false,
      "Opponents' postcombat main stop: on for one turn. Press to turn it off",
    ],
    ['Opponent', 'End', 'lasting', true, "Opponents' end step stop: on every turn. Press to turn it off"],
  ] satisfies [StopSide, Step, StopState, boolean, string][])(
    'says %s %s %s (keep on: %s) as "%s"',
    (side, step, state, lasting, words) => {
      expect(stopName(side, step, state, lasting)).toBe(words)
    },
  )

  it.each(
    SIDES.flatMap((side) =>
      STATES.flatMap((state) =>
        STOP_STEPS.map((step): [StopSide, StopState, StopStep] => [side, state, step]),
      ),
    ),
  )('names %s %s %s with its side, its step, its state and what pressing does', (side, state, step) => {
    const name = stopName(side, step, state, false)
    expect(name).toContain(side === 'Own' ? 'Your ' : "Opponents' ")
    expect(name).toContain(` ${stepWords(step)} stop: `)
    expect(name).toMatch(/: (off|on for one turn|on every turn)\. Press to \S/)
  })

  it('never says "pass priority", which is not how a player talks about a stop', () => {
    for (const side of SIDES) {
      for (const state of STATES) expect(stopName(side, 'Main1', state, false)).not.toMatch(/priority/i)
    }
  })
})

describe('the other kind of press', () => {
  it.each([
    ['Own', 'off', false, 'Shift and press to stop here every turn.'],
    ['Own', 'off', true, 'Shift and press to stop here this turn.'],
    ['Own', 'turn', false, 'Shift and press to keep it every turn.'],
    ['Own', 'turn', true, 'Shift and press to turn it off.'],
    ['Opponent', 'off', false, 'Shift and press to stop here on every opponent turn.'],
    ['Opponent', 'off', true, 'Shift and press to stop here on one opponent turn.'],
  ] satisfies [StopSide, StopState, boolean, string][])(
    'says, for %s %s with Keep %s, "%s"',
    (side, state, lasting, words) => {
      expect(stopDescription(side, state, lasting)).toBe(words)
    },
  )

  it.each(SIDES.flatMap((side) => [true, false].map((lasting): [StopSide, boolean] => [side, lasting])))(
    'is left out for a lasting stop on %s with Keep %s, where both presses turn it off',
    (side, lasting) => {
      expect(stopDescription(side, 'lasting', lasting)).toBeUndefined()
    },
  )
})

describe('the words around the buttons', () => {
  it.each([
    ['Own', false, 'Stops on your turns'],
    ['Own', true, 'Stops on your turns, your turn is under way'],
    ['Opponent', false, "Stops on opponents' turns"],
    ['Opponent', true, "Stops on opponents' turns, an opponent's turn is under way"],
  ] satisfies [StopSide, boolean, string][])(
    'name the row of %s with its turn %s: "%s"',
    (side, turn, name) => {
      expect(rowName(side, turn)).toBe(name)
    },
  )

  it.each([
    [0, 'none set'],
    [1, '1 set'],
    [7, '7 set'],
  ])('count %i stops as "%s"', (count, words) => {
    expect(stopCount(count)).toBe(words)
  })
})
