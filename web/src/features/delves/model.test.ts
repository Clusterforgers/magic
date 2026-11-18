// Goal: the run page marks each room of the ladder cleared, current, next or locked through every stage of a run, names who voted for which camp, gives a boss its life for the party from the ladder the run lists, words each Heat as its tier, tones life bars, words the stage, knows a hero ready once they bring a deck, reads how the last fight was lost, words the loot before a descent apart from a camp, tells the first boss from a later one for the life the party regroups at.
import { describe, expect, it } from 'vitest'
import type { Run } from '@/protocol/generated'
import { hero, run } from './fakeDelves'
import {
  atFirstBoss,
  bossLife,
  heatTier,
  heatWords,
  isReady,
  lastFight,
  lifeShare,
  lifeTone,
  nextStep,
  nodeState,
  openChoices,
  regroupLife,
  runStage,
  votersOf,
} from './model'
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

it('reads a boss life for the party from the ladder the run lists', () => {
  const view = {
    boss_lives: [
      ['skrix', 18],
      ['grimgut', 33],
    ] as [string, number][],
  }
  expect(bossLife(view, 'grimgut')).toBe(33)
  expect(bossLife(view, 'vex')).toBeUndefined()
})

it.each([
  [0, 'Heat 0 — Casual: any deck can win with some luck'],
  [1, 'Heat 1 — Seasoned: needs a deck that suits the format'],
  [2, 'Heat 2 — Heroic: needs a team of decks that work together and suit the format'],
] as const)('words Heat %i as its tier', (heat, words) => {
  expect(heatWords(heat)).toBe(words)
  expect(heatTier(heat).heat).toBe(heat)
})

it('reads a Heat beyond the table as the highest tier', () => {
  expect(heatTier(5).name).toBe('Heroic')
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
  expect(isReady(hero({ deck: { id: 'd', name: 'D', colors: 'G', commanders: [] } }))).toBe(true)
})

it.each([
  ['The party was beaten back and regroups (2 hearts left).', 'lost'],
  ['The party retreated and regroups (2 hearts left).', 'retreated'],
  ['The party lost its table and regroups (2 hearts left).', 'closed'],
  ['Skrix awaits.', null],
] as const)('reads the last line of a fight that was lost: %s', (line, how) => {
  expect(lastFight(run({ status: { Fight: { game: null } }, at: 0, log: ['Ana joined', line] }))).toBe(how)
  // The same line at a camp is no longer news.
  expect(lastFight(run({ status: 'Map', at: 0, log: [line] }))).toBeNull()
})

it.each([
  ['nowhere yet', null, true, 'full'],
  ['at the first boss', 0, true, 'full'],
  ['at the shop between the bosses', 1, false, 'half'],
  ['at the event between the bosses', 2, false, 'half'],
  ['at the later boss', 3, false, 'half'],
  ['at a room the map lacks', 9, true, 'full'],
] as const)('tells the first boss from a later one, %s', (_where, at, first, life) => {
  const r = run({ at })
  expect(atFirstBoss(r)).toBe(first)
  expect(regroupLife(r)).toBe(life)
})

it('reads the first boss by the rows of the map, not by the order of its nodes', () => {
  const nodes = run().map.nodes
  const reversed = run({ at: 3, map: { nodes: [...nodes].reverse() } })
  expect(atFirstBoss(reversed)).toBe(false)
  // A ladder whose first boss room lies in row 1 has its camp in row 0: that boss is the first.
  const camp = { id: 0, row: 0, lane: 0, kind: 'shop' as const, next: [1] }
  const boss = { id: 1, row: 1, lane: 0, kind: 'boss' as const, boss: 'skrix_nest', next: [2] }
  const last = { id: 2, row: 2, lane: 0, kind: 'boss' as const, boss: 'grimgut', next: [] }
  expect(regroupLife(run({ at: 1, map: { nodes: [camp, boss, last] } }))).toBe('full')
  expect(regroupLife(run({ at: 2, map: { nodes: [camp, boss, last] } }))).toBe('half')
})

it('reads a boss that fell as the last fight won', () => {
  expect(lastFight(run({ status: 'Room', at: 0, cleared: [0] }))).toBe('won')
})

it.each([
  [false, 'At camp', 'after a boss it votes for a camp'],
  [true, 'Loot before the descent', 'descends to the next floor'],
])('words the loot of a descent apart from a camp (descending %s)', (descending, stage, step) => {
  const view = delveView({}, { status: 'Room', at: 3, cleared: [0, 3], floors_cleared: 1, descending })
  expect(runStage(view)).toBe(stage)
  expect(nextStep(view)).toContain(step)
  expect(lastFight(view.run)).toBe('won')
})
