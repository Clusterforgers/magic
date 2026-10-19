// Goal: the open-games words are right at every boundary (ages, seats, counts, the Join button's name, the format key), the filters keep exactly the rows they name in their order, and a typed join code is read the way the server reads it.
import { describe, expect, it } from 'vitest'
import type { LobbyGame } from '@/protocol/generated'
import {
  ageNow,
  ageText,
  filterGames,
  filtersActive,
  formatName,
  formatOptions,
  isFull,
  isJoinCode,
  joinLabel,
  nextJoin,
  normalizeCode,
  NO_FILTERS,
  openGamesText,
  seatOptions,
  seatsText,
} from './format'

const game = (over: Partial<LobbyGame> = {}): LobbyGame => ({
  game: 'g1',
  host: 'Bo',
  format: 'normal',
  filled: 2,
  total: 4,
  created_at: '2026-10-19T12:00:00Z',
  age_secs: 180,
  ...over,
})

describe('ageText', () => {
  it.each([
    [0, 'just now'],
    [59, 'just now'],
    [60, '1 min ago'],
    [179, '2 min ago'],
    [180, '3 min ago'],
    [3599, '59 min ago'],
    [3600, '1 h ago'],
    [86_399, '23 h ago'],
    [86_400, '1 d ago'],
    [200_000, '2 d ago'],
  ])('says %i seconds as "%s"', (seconds, text) => {
    expect(ageText(seconds)).toBe(text)
  })
})

describe('ageNow', () => {
  it.each([
    ['at the moment the rows arrived', 5_000, 5_000, 180],
    ['a minute later', 5_000, 65_000, 240],
    ['whole seconds only', 5_000, 65_999, 240],
    ['a clock that is behind the arrival never makes a row younger', 5_000, 1_000, 180],
  ])('counts a row %s', (_what, fetchedAt, now, seconds) => {
    expect(ageNow(game({ age_secs: 180 }), fetchedAt, now)).toBe(seconds)
  })
})

describe('formatName', () => {
  it.each([
    ['normal', 'Normal'],
    ['commander', 'Commander'],
    ['free-for-all', 'Free for all'],
    ['two_headed', 'Two headed'],
    ['', ''],
  ])('writes the key "%s" as "%s"', (key, name) => {
    expect(formatName(key)).toBe(name)
  })
})

describe('seats and the Join button', () => {
  it.each([
    [2, 4, '2 of 4', false],
    [4, 4, '4 of 4', true],
    [1, 12, '1 of 12', false],
  ])('%i of %i seats reads "%s" and full is %s', (filled, total, text, full) => {
    const row = game({ filled, total })
    expect(seatsText(row)).toBe(text)
    expect(isFull(row)).toBe(full)
  })

  it('names the button after the host and the seats', () => {
    expect(joinLabel(game())).toBe("Join Bo's game, 2 of 4 seats")
    expect(joinLabel(game({ host: 'Ann Lee', filled: 3, total: 3 }))).toBe(
      "Join Ann Lee's game, 3 of 3 seats",
    )
  })
})

describe('openGamesText', () => {
  it.each([
    [0, 0, '0 open games'],
    [1, 1, '1 open game'],
    [3, 3, '3 open games'],
    [2, 12, '2 of 12 open games'],
    [0, 12, '0 of 12 open games'],
    [1, 2, '1 of 2 open games'],
  ])('says %i shown of %i as "%s"', (shown, all, text) => {
    expect(openGamesText(shown, all)).toBe(text)
  })
})

describe('filters', () => {
  const rows = [
    game({ game: 'a', total: 2, filled: 1, format: 'normal' }),
    game({ game: 'b', total: 4, filled: 4, format: 'normal' }),
    game({ game: 'c', total: 4, filled: 2, format: 'commander' }),
    game({ game: 'd', total: 2, filled: 2, format: 'commander' }),
    game({ game: 'e', total: 6, filled: 3, format: 'normal' }),
  ]
  const ids = (filters: Partial<typeof NO_FILTERS>): string[] =>
    filterGames(rows, { ...NO_FILTERS, ...filters }).map((row) => row.game)

  it.each([
    ['no filter keeps every row in order', {}, ['a', 'b', 'c', 'd', 'e']],
    ['a seat count keeps games of exactly that size', { seats: 4 }, ['b', 'c']],
    ['a format keeps that format', { format: 'commander' }, ['c', 'd']],
    ['hide full drops the games with no open seat', { hideFull: true }, ['a', 'c', 'e']],
    ['all three together keep what passes every one', { seats: 2, format: 'commander', hideFull: true }, []],
    ['a seat count and a format', { seats: 4, format: 'normal' }, ['b']],
    ['a seat count nobody has', { seats: 9 }, []],
  ])('%s', (_what, filters, expected) => {
    expect(ids(filters)).toEqual(expected)
  })

  it('lists the seat counts and formats that exist, in a fixed order', () => {
    expect(seatOptions(rows)).toEqual([2, 4, 6])
    expect(formatOptions(rows)).toEqual(['commander', 'normal'])
    expect(seatOptions([])).toEqual([])
  })

  it('knows when a filter is set', () => {
    expect(filtersActive(NO_FILTERS)).toBe(false)
    expect(filtersActive({ ...NO_FILTERS, seats: 2 })).toBe(true)
    expect(filtersActive({ ...NO_FILTERS, format: 'normal' })).toBe(true)
    expect(filtersActive({ ...NO_FILTERS, hideFull: true })).toBe(true)
  })
})

describe('nextJoin', () => {
  const rows = ['a', 'b', 'c', 'd'].map((id) => game({ game: id }))
  const full = (id: string) => game({ game: id, filled: 4, total: 4 })
  const of = (...ids: string[]) => ids.map((id) => rows.find((row) => row.game === id) as LobbyGame)

  it.each([
    ['the nearest row after', 'b', ['a', 'c', 'd'], 'c'],
    ['the first row when the first went', 'a', ['b', 'c', 'd'], 'b'],
    ['the nearest row before when the last went', 'd', ['a', 'b', 'c'], 'c'],
    ['the next one still there when the one after went too', 'b', ['a', 'd'], 'd'],
    ['the one before when none after is left', 'c', ['a', 'b'], 'b'],
  ])('is %s', (_what, gone, left, expected) => {
    expect(nextJoin(rows, gone, of(...left))).toBe(expected)
  })

  it('prefers a game that can be joined, after the gone row first and then before it', () => {
    expect(nextJoin(rows, 'b', [rows[0] as LobbyGame, full('c'), rows[3] as LobbyGame])).toBe('d')
    expect(nextJoin(rows, 'c', [rows[0] as LobbyGame, rows[1] as LobbyGame, full('d')])).toBe('b')
  })

  it('falls back to a full game when it is all that is left, and to nothing when no row is', () => {
    expect(nextJoin(rows, 'a', [full('b'), full('c')])).toBe('b')
    expect(nextJoin(rows, 'a', [])).toBeNull()
  })

  it('knows nothing of a row it was not shown, and still answers with the first row', () => {
    expect(nextJoin(rows, 'zzz', of('a', 'b'))).toBe('a')
  })
})

describe('join codes', () => {
  it.each([
    ['abcd2345', 'ABCD2345'],
    ['abcd-2345', 'ABCD2345'],
    ['  ab cd 23 45 ', 'ABCD2345'],
    ['AB-CD-23-45', 'ABCD2345'],
    ['', ''],
  ])('reads "%s" as "%s"', (typed, code) => {
    expect(normalizeCode(typed)).toBe(code)
  })

  it.each([
    ['ABCD2345', true],
    ['KMNPQRST', true],
    ['23456789', true],
    ['ABCD234', false],
    ['ABCD23456', false],
    ['ABCD234I', false],
    ['ABCD234O', false],
    ['ABCD2340', false],
    ['ABCD2341', false],
    ['abcd2345', false],
    ['', false],
  ])('%s is a code: %s', (code, valid) => {
    expect(isJoinCode(code)).toBe(valid)
  })
})
