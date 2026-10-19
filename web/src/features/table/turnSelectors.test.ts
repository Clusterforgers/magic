// Goal: the turn selectors read the stage, the result, the active seat and the priority holder alone: turns run at the stage Turns while the game has no result, a seat is the active one only then and only if the view names it, the viewer's turn is that for the viewer's own seat, and priority is whoever the view names, so every seat that looks draws the same marks.
import { describe, expect, it } from 'vitest'
import type { PlayState } from '@/play/store'
import type { GameResult, Stage } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { fixtureState } from './dev/devGame'
import { buildFixture } from './dev/fixture'
import { holdsPriority, isActiveSeat, isYourTurn, turnsRunning } from './selectors'

/** A four-seat game as seat 0 sees it: Bo (seat 1) has the turn and Alice, the viewer, holds priority. */
function state(edit: (state: PlayState) => void = () => undefined): PlayState {
  const first = fixtureState(buildFixture({ seats: 4, permanents: 2 }))
  const next = { ...first, view: clone(must(first.view)) }
  edit(next)
  return next
}

const STAGES: Array<[Stage, GameResult | null, boolean]> = [
  ['Turns', null, true],
  ['Mulligan', null, false],
  ['PreGame', null, false],
  ['Finished', { Winner: 0 }, false],
  ['Turns', 'Draw', false],
  ['Finished', null, false],
]

describe('turnsRunning', () => {
  it.each(STAGES)('is decided by the stage %s and the result %j: %s', (stage, result, running) => {
    const game = state((draft) => {
      must(draft.view).stage = stage
      must(draft.view).result = result
    })
    expect(turnsRunning(game)).toBe(running)
  })

  it('is false while there is no view yet', () => {
    expect(turnsRunning({ ...state(), view: null })).toBe(false)
  })
})

describe('isActiveSeat', () => {
  it.each(STAGES)('names the seat of the turn only while turns run (%s, %j)', (stage, result, running) => {
    const game = state((draft) => {
      must(draft.view).stage = stage
      must(draft.view).result = result
    })
    expect([0, 1, 2, 3].map((seat) => isActiveSeat(game, seat))).toEqual(
      running ? [false, true, false, false] : [false, false, false, false],
    )
  })

  it('follows the active seat of the view, whoever looks', () => {
    for (const viewer of [0, 1, 3]) {
      const game = state((draft) => {
        draft.seat = viewer
        must(draft.view).turn.active = 2
      })
      expect([0, 1, 2, 3].filter((seat) => isActiveSeat(game, seat))).toEqual([2])
    }
  })

  it('is false for every seat while there is no view', () => {
    expect(isActiveSeat({ ...state(), view: null }, 1)).toBe(false)
  })
})

describe('isYourTurn', () => {
  it.each([
    [0, 0, true],
    [0, 1, false],
    [1, 1, true],
    [3, 1, false],
    [null, 1, false],
  ])('is %s watching the turn of seat %s: %s', (viewer, active, expected) => {
    const game = state((draft) => {
      draft.seat = viewer
      must(draft.view).turn.active = active
    })
    expect(isYourTurn(game)).toBe(expected)
  })

  it('is false once the result is in, though the view still names the viewer', () => {
    const game = state((draft) => {
      must(draft.view).turn.active = 0
      must(draft.view).result = 'Draw'
    })
    expect(isYourTurn(game)).toBe(false)
  })
})

describe('holdsPriority', () => {
  it('names the holder of the view and no other seat, at any stage', () => {
    for (const stage of ['Turns', 'Mulligan', 'Finished'] as const) {
      const game = state((draft) => {
        must(draft.view).stage = stage
        must(draft.view).priority = { holder: 2, passed: [1] }
      })
      expect([0, 1, 2, 3].filter((seat) => holdsPriority(game, seat))).toEqual([2])
    }
  })

  it('names nobody while the view has no priority, or no view', () => {
    const none = state((draft) => {
      must(draft.view).priority = null
    })
    expect([0, 1, 2, 3].some((seat) => holdsPriority(none, seat))).toBe(false)
    expect(holdsPriority({ ...state(), view: null }, 0)).toBe(false)
  })
})
