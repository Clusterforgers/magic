// Goal: the run page marks each room of the ladder cleared, current, next or locked through every stage of a run, names who voted for which camp, gives a boss its life for the party and Heat, tones life bars, words the stage and knows a hero ready once they bring a deck.
import { describe, expect, it } from 'vitest'
import type { Run } from '@/protocol/generated'
import { hero, run } from './fakeDelves'
import { bossLife, isReady, lifeShare, lifeTone, nodeState, openChoices, runStage, votersOf } from './model'
import { delveView } from './fakeDelves'

// The test ladder: boss 0, then the shop 1 or the event 2, then boss 3.
const states = (over: Partial<Run>) => {
  const r = run(over)
  return r.map.nodes.map((node) => nodeState(node, r))
}

describe('nodeState', () => {
  it.each([
    ['in the lobby', { status: 'Lobby' as const }, ['locked', 'locked', 'locked', 'locked']],
    [
      'fighting the first boss',
      { status: { Fight: { game: 'g' } }, at: 0 },
      ['current', 'locked', 'locked', 'locked'],
    ],
    [
      'looting the first boss',
      { status: 'Room' as const, at: 0, cleared: [0] },
      ['current', 'locked', 'locked', 'locked'],
    ],
    [
      'voting on the camps',
      { status: 'Map' as const, at: 0, cleared: [0] },
      ['cleared', 'next', 'next', 'locked'],
    ],
    [
      'at the shop',
      { status: 'Room' as const, at: 1, cleared: [0] },
      ['cleared', 'current', 'locked', 'locked'],
    ],
    [
      'past the shop',
      { status: 'Map' as const, at: 1, cleared: [0, 1] },
      ['cleared', 'cleared', 'locked', 'next'],
    ],
    [
      'won',
      { status: 'Victory' as const, at: 3, cleared: [0, 1, 3] },
      ['cleared', 'cleared', 'locked', 'cleared'],
    ],
    [
      'fallen at the last boss',
      { status: 'Defeat' as const, at: 3, cleared: [0, 2] },
      ['cleared', 'locked', 'cleared', 'current'],
    ],
  ])('%s', (_, over, expected) => {
    expect(states(over as Partial<Run>)).toEqual(expected)
  })

  it('opens choices only while the party chooses a path', () => {
    expect(openChoices(run({ status: 'Map', at: 0 }))).toEqual([1, 2])
    expect(openChoices(run({ status: 'Room', at: 0 }))).toEqual([])
    expect(openChoices(run({ status: 'Map', at: null }))).toEqual([])
  })
})

it('names the voters of each room', () => {
  const r = run({ heroes: [hero(), hero({ name: 'Bo' }), hero({ name: 'Cy' })], votes: { 0: 1, 1: 2, 2: 1 } })
  expect(votersOf(r, 1)).toEqual(['Ana', 'Cy'])
  expect(votersOf(r, 2)).toEqual(['Bo'])
  expect(votersOf(r, 3)).toEqual([])
})

it.each([
  [[30, 8], 1, 0, 30],
  [[30, 8], 4, 0, 54],
  [[30, 8], 1, 1, 36],
  [[30, 8], 2, 2, 46],
] as const)('a boss of life %j with %i heroes at Heat %i has %i life', (life, heroes, heat, expected) => {
  expect(bossLife(life, heroes, heat)).toBe(expected)
})

it.each([
  [40, 'fine', 1],
  [20, 'fine', 0.5],
  [19, 'hurt', 0.475],
  [10, 'low', 0.25],
  [0, 'low', 0],
  [55, 'fine', 1],
])('a life of %i is %s and fills %f of the bar', (life, tone, share) => {
  expect(lifeTone(life)).toBe(tone)
  expect(lifeShare(life)).toBe(share)
})

it.each([
  ['Lobby', 'Gathering the party'],
  ['Map', 'Choosing a path'],
  ['Room', 'At camp'],
  ['Victory', 'Victory'],
  ['Defeat', 'Defeated'],
  ['Abandoned', 'Turned back'],
] as const)('words the stage %s', (status, words) => {
  expect(runStage(delveView({}, { status }))).toBe(words)
  expect(runStage(delveView({}, { status: { Fight: { game: 'g' } } }))).toBe('Boss fight')
})

it('knows a hero ready once they bring a deck', () => {
  expect(isReady(hero())).toBe(false)
  expect(isReady(hero({ deck: { id: 'd', name: 'D', colors: 'G' } }))).toBe(true)
})
