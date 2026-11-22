// Goal: the Card support words read right: shares and counts, a line per feed event (precons per set, new cards, a release with the cards it made automatic, a failed check), the daily check's state, the search over names and blockers without case or accents, and the trend's points inside their box.
import { describe, expect, it } from 'vitest'
import type { FeedEvent, MissingCardView, RefreshCheck, SupportPoint } from '@/protocol/generated'
import { checkWords, feedLine, filterMissing, percent, someNames, stateCounts, trendPoints } from './model'

const at = '2026-11-22T04:00:00Z'

describe('feed lines', () => {
  it.each<[FeedEvent, string, string]>([
    [
      { kind: 'new_precons', at, code: 'STC', set_name: 'Star Trek Commander', decks: ['A', 'B', 'C', 'D'] },
      'Star Trek Commander: 4 precons added',
      'A, B, C, D',
    ],
    [
      { kind: 'new_precons', at, code: 'XYZ', set_name: null, decks: ['Solo'] },
      'XYZ: 1 precon added',
      'Solo',
    ],
    [
      { kind: 'new_cards', at, count: 289, names: ['Kirk', 'Spock'] },
      '289 new cards',
      'Kirk, Spock and 287 more',
    ],
    [
      { kind: 'now_automatic', at, release: 'v0.2.131', new_release: true, count: 170, names: [] },
      'v0.2.131: 170 cards now automatic',
      '',
    ],
    [
      { kind: 'now_automatic', at, release: 'v0.2.131', new_release: false, count: 1, names: ['Bolt'] },
      '1 card now automatic',
      'Bolt',
    ],
    [
      { kind: 'now_automatic', at, release: 'v0.2.132', new_release: true, count: 0, names: [] },
      'v0.2.132 released',
      '',
    ],
    [{ kind: 'check_failed', at, failures: ['fetch: offline'] }, 'The daily check failed', 'fetch: offline'],
  ])('%j reads %s', (event, title, detail) => {
    expect(feedLine(event)).toMatchObject({ title, detail, at, kind: event.kind })
  })

  it('lists five names at most', () => {
    expect(someNames(['a', 'b', 'c', 'd', 'e', 'f'], 6)).toBe('a, b, c, d, e and 1 more')
    expect(someNames([], 4)).toBe('')
  })
})

describe('numbers', () => {
  it.each([
    [947, 1000, '94.7%'],
    [1, 3, '33.3%'],
    [0, 0, '0%'],
  ])('%i of %i is %s', (part, whole, text) => {
    expect(percent(part, whole)).toBe(text)
  })
})

describe('the daily check', () => {
  const now = Date.parse('2026-11-22T07:00:00Z')
  const check = (state: RefreshCheck['state']): RefreshCheck => ({
    started_at: '2026-11-22T03:59:00Z',
    finished_at: state === 'running' ? null : at,
    state,
    scryfall_updated_at: null,
    mtgjson_build: null,
  })
  it.each<[RefreshCheck['state'], string]>([
    ['succeeded', 'Checked for new cards and precons 3 hours ago.'],
    ['deferred', 'Checked 3 hours ago; new cards load once no game is running.'],
    ['failed', 'The last check failed 3 hours ago; the numbers are from the check before.'],
    ['running', 'Checking for new cards and precons now…'],
  ])('%s reads its words', (state, words) => {
    expect(checkWords(check(state), now)).toBe(words)
  })
  it('says nothing before the first check', () => {
    expect(checkWords(null, now)).toBeNull()
  })
})

describe('the missing cards', () => {
  const card = (name: string, state: MissingCardView['state'], blocker: string): MissingCardView => ({
    oracle_id: name,
    name,
    state,
    blocker,
    detail: '',
    blockers: 1,
    rank: null,
  })
  const cards = [
    card('Æther Vial', 'partial', 'A triggered ability the game cannot read yet'),
    card('Chaos Orb', 'manual', 'No rules yet'),
    card('Lim-Dûl', 'waiting', 'Waits for a player to read its rules'),
  ]
  it.each<[string, MissingCardView['state'] | 'all', string[]]>([
    ['', 'all', ['Æther Vial', 'Chaos Orb', 'Lim-Dûl']],
    ['aether', 'all', ['Æther Vial']],
    ['orb vial', 'all', []],
    ['lim-dul', 'all', ['Lim-Dûl']],
    ['TRIGGERED', 'all', ['Æther Vial']],
    ['', 'manual', ['Chaos Orb']],
    ['rules', 'waiting', ['Lim-Dûl']],
  ])('%j in %s finds %j', (query, state, names) => {
    expect(filterMissing(cards, { query, state }).map((c) => c.name)).toEqual(names)
  })
  it('counts the states', () => {
    expect(stateCounts(cards)).toEqual({ waiting: 1, partial: 1, manual: 1, other: 0 })
  })
})

describe('the trend', () => {
  const point = (day: number, automatic: number): SupportPoint => ({
    taken_at: `2026-11-${String(day).padStart(2, '0')}T04:00:00Z`,
    release: null,
    cards: 100,
    automatic,
  })
  it('spreads the points by time and puts the lowest share above the floor', () => {
    const { points, floor } = trendPoints([point(1, 90), point(2, 92), point(11, 95)], 100, 100)
    expect(floor).toBe(0.85)
    expect(points.map((p) => Math.round(p.x))).toEqual([0, 10, 100])
    expect(points.every((p) => p.y >= 0 && p.y <= 100)).toBe(true)
    expect(points[2]?.y).toBeCloseTo(100 - (0.1 / 0.15) * 100)
  })
  it('puts one point in the middle', () => {
    expect(trendPoints([point(1, 50)], 100, 100).points[0]?.x).toBe(50)
  })
})
