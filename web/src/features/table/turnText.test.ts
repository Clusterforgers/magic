// Goal: the turn banner, priority line, mulligan progress and the read-only "what is the game waiting for" line say the right thing in every stage, step and decision kind.
import { describe, expect, it } from 'vitest'
import type { PlayState } from '@/play/store'
import type { MulliganStatus, Stage, Step } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { KINDS } from '@/test-utils/decisions'
import { fixtureState } from './dev/devGame'
import { buildFixture } from './dev/fixture'
import {
  badgeText,
  bannerText,
  DECISION_LABEL,
  decisionName,
  describeMulligan,
  priorityText,
  STEP_LABEL,
  waitingOn,
  waitingText,
} from './turnText'
import type { BannerInput, DecisionName } from './turnText'

const NAMES = ['Alice', 'Bo', 'Cy', 'Dee', 'Eli']
const name = (seat: number): string => NAMES[seat] ?? `Seat ${seat + 1}`

// One of every decision kind, from the shared samples: the type makes a new kind fail to compile there until it has one.
const NAMES_OF_KINDS = Object.keys(KINDS) as DecisionName[]
const STEPS: Step[] = [
  'Untap',
  'Upkeep',
  'Draw',
  'Main1',
  'BeginCombat',
  'DeclareAttackers',
  'DeclareBlockers',
  'FirstStrikeDamage',
  'CombatDamage',
  'EndCombat',
  'Main2',
  'End',
  'Cleanup',
]

const base = fixtureState(buildFixture({ seats: 4 }))

/** The fixture state with the view changed by `edit` and no pending decision. */
function quiet(edit: (view: NonNullable<PlayState['view']>) => void): PlayState {
  const view = clone(must(base.view))
  edit(view)
  return { ...base, view, pending: null }
}

describe('bannerText', () => {
  const turn = (step: Step): BannerInput => ({ stage: 'Turns', turn: 5, active: 0, step })

  it.each(STEPS)('names the %s step in the banner with its own label', (step) => {
    expect(STEP_LABEL[step]).not.toBe('')
    expect(bannerText(turn(step), name)).toBe(`Turn 5 · Alice · ${STEP_LABEL[step]}`)
  })

  it('has a distinct label for every step', () => {
    expect(new Set(STEPS.map((step) => STEP_LABEL[step])).size).toBe(STEPS.length)
  })

  it('reads like the example: turn, player, step', () => {
    expect(bannerText({ stage: 'Turns', turn: 12, active: 2, step: 'DeclareAttackers' }, name)).toBe(
      'Turn 12 · Cy · Declare attackers',
    )
  })

  it.each<[Stage, string]>([
    ['PreGame', 'Setting up the game'],
    ['Mulligan', 'Opening hands'],
    ['Finished', 'The game is over'],
  ])('says what the game is doing in stage %s', (stage, text) => {
    expect(bannerText({ stage, turn: 0, active: 0, step: 'Untap' }, name)).toBe(text)
  })
})

describe('badgeText', () => {
  const turn = (active: number, step: Step): BannerInput => ({ stage: 'Turns', turn: 5, active, step })

  it.each(STEPS)('says "Your turn" with the %s step on the viewer\'s own turn', (step) => {
    expect(badgeText(turn(0, step), 0, name)).toBe(`Your turn · 5 · ${STEP_LABEL[step]}`)
  })

  it.each(STEPS)("names the other player and the %s step on an opponent's turn", (step) => {
    expect(badgeText(turn(1, step), 0, name)).toBe(`Bo's turn · 5 · ${STEP_LABEL[step]}`)
  })

  it.each([0, 1, 2, 3, 4])('says "Your turn" only for the viewer, seat %i', (viewer) => {
    const words = [0, 1, 2, 3, 4].map((active) => badgeText(turn(active, 'Main1'), viewer, name))
    expect(words.filter((text) => text.startsWith('Your turn'))).toHaveLength(1)
    expect(words[viewer]).toBe('Your turn · 5 · Precombat main')
  })

  it('names the active player when the viewer is not known yet', () => {
    expect(badgeText(turn(0, 'Main1'), null, name)).toBe("Alice's turn · 5 · Precombat main")
  })

  it.each<[Stage, string]>([
    ['PreGame', 'Setting up the game'],
    ['Mulligan', 'Opening hands'],
    ['Finished', 'The game is over'],
  ])('says what the game is doing in stage %s, whoever is looking', (stage, text) => {
    expect(badgeText({ stage, turn: 0, active: 0, step: 'Untap' }, 0, name)).toBe(text)
    expect(badgeText({ stage, turn: 0, active: 0, step: 'Untap' }, 1, name)).toBe(text)
  })
})

describe('priorityText', () => {
  it('says who holds priority and how many have passed', () => {
    expect(priorityText({ holder: 1, passed: [] }, 'Turns', name)).toBe('Bo has priority')
    expect(priorityText({ holder: 1, passed: [0, 2] }, 'Turns', name)).toBe('Bo has priority · 2 passed')
  })

  it('says nobody has it during a turn, and nothing outside one', () => {
    expect(priorityText(null, 'Turns', name)).toBe('Nobody has priority')
    for (const stage of ['PreGame', 'Mulligan', 'Finished'] as const)
      expect(priorityText(null, stage, name)).toBe('')
  })
})

describe('describeMulligan', () => {
  it.each<[MulliganStatus, number, string]>([
    ['NeedsHand', 0, 'Waiting for a new hand'],
    ['Deciding', 0, 'Deciding whether to keep'],
    ['Mulliganing', 1, 'Mulligan taken, waiting for the table · 1 mulligan'],
    ['Kept', 2, 'Kept their hand · 2 mulligans'],
    [{ Bottoming: 1 }, 1, 'Putting 1 card on the bottom · 1 mulligan'],
    [{ Bottoming: 3 }, 3, 'Putting 3 cards on the bottom · 3 mulligans'],
  ])('describes %j after %i mulligans', (status, mulligans, text) => {
    expect(describeMulligan(status, mulligans)).toBe(text)
  })
})

describe('decisionName', () => {
  it.each(NAMES_OF_KINDS)('names a %s decision', (kind) => {
    expect(decisionName(KINDS[kind])).toBe(kind)
    expect(DECISION_LABEL[kind]).not.toBe('')
  })
})

describe('waitingOn', () => {
  it.each(NAMES_OF_KINDS)("is this seat's own %s decision when one is pending", (kind) => {
    const state: PlayState = { ...base, pending: { id: 3, decision: { player: 0, kind: KINDS[kind] } } }
    const waiting = waitingOn(state)
    expect(waiting).toEqual({ kind: 'you', label: DECISION_LABEL[kind] })
    expect(waitingText(waiting, name)).toBe(`Your decision: ${DECISION_LABEL[kind]}`)
  })

  it('waits for the priority holder when it is somebody else', () => {
    const state = quiet((view) => {
      view.priority = { holder: 2, passed: [] }
    })
    expect(waitingOn(state)).toEqual({ kind: 'others', seats: [2] })
    expect(waitingText(waitingOn(state), name)).toBe('Waiting for Cy')
  })

  it('has nothing to wait for when this seat holds priority but its decision has not arrived', () => {
    const state = quiet((view) => {
      view.priority = { holder: 0, passed: [] }
    })
    expect(waitingOn(state)).toEqual({ kind: 'idle', label: 'Waiting for the game' })
  })

  it('waits for the active player to declare attackers', () => {
    const state = quiet((view) => {
      view.priority = null
      view.turn = { ...view.turn, active: 3, step: 'DeclareAttackers' }
      view.combat = { attackers: [], attackers_declared: false, blockers_declared: [] }
    })
    expect(waitingOn(state)).toEqual({ kind: 'others', seats: [3] })
  })

  it('waits for the seats still choosing their opening hand, never for this one or an eliminated one', () => {
    const state = quiet((view) => {
      view.stage = 'Mulligan'
      view.priority = null
      const statuses: MulliganStatus[] = ['Deciding', 'Deciding', { Bottoming: 1 }, 'Kept']
      view.players.forEach((player, index) => {
        player.mulligan = statuses[index] ?? 'Kept'
      })
      must(view.players[1]).lost = 'Conceded'
    })
    expect(waitingOn(state)).toEqual({ kind: 'others', seats: [2] })
  })

  it('has nothing to wait for once every seat has kept', () => {
    const state = quiet((view) => {
      view.stage = 'Mulligan'
      view.priority = null
    })
    expect(waitingOn(state).kind).toBe('idle')
  })

  it('says the game is over when the stage is Finished or there is a result', () => {
    const over = { kind: 'idle', label: 'The game is over' }
    expect(
      waitingOn(
        quiet((view) => {
          view.stage = 'Finished'
        }),
      ),
    ).toEqual(over)
    expect(
      waitingOn(
        quiet((view) => {
          view.result = 'Draw'
        }),
      ),
    ).toEqual(over)
  })

  it('has nothing to wait for before there is a view', () => {
    expect(waitingOn({ ...base, view: null, pending: null })).toEqual({
      kind: 'idle',
      label: 'Waiting for the game',
    })
  })
})

describe('waitingText', () => {
  it.each([
    [[1], 'Waiting for Bo'],
    [[1, 2], 'Waiting for Bo and Cy'],
    [[1, 2, 3], 'Waiting for Bo, Cy and Dee'],
    [[1, 2, 3, 4], 'Waiting for 4 players'],
  ])('lists %j as "%s"', (seats, text) => {
    expect(waitingText({ kind: 'others', seats }, name)).toBe(text)
  })

  it('prints an idle label as it is', () => {
    expect(waitingText({ kind: 'idle', label: 'The game is over' }, name)).toBe('The game is over')
  })
})
