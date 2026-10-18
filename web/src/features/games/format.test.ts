// Goal: the words of the games list are right for every age, every set of other players and every way a game can end, whoever won and however many seats there were.
import { describe, expect, it } from 'vitest'
import { listed } from '@/test-utils/accountsServer'
import { joinNames, othersText, playerLabel, resultText, timeSince } from './format'

const NOW = Date.parse('2026-10-18T12:00:00Z')
const ago = (ms: number): string => new Date(NOW - ms).toISOString().replace(/\.\d{3}Z$/, 'Z')
const SECOND = 1000
const MINUTE = 60 * SECOND
const HOUR = 60 * MINUTE
const DAY = 24 * HOUR

describe('timeSince', () => {
  it.each([
    [0, 'just now'],
    [30 * SECOND, 'just now'],
    [59 * SECOND, 'just now'],
    [MINUTE, '1 minute ago'],
    [90 * SECOND, '1 minute ago'],
    [2 * MINUTE, '2 minutes ago'],
    [59 * MINUTE + 59 * SECOND, '59 minutes ago'],
    [HOUR, '1 hour ago'],
    [HOUR + 59 * MINUTE, '1 hour ago'],
    [5 * HOUR, '5 hours ago'],
    [23 * HOUR + 59 * MINUTE, '23 hours ago'],
    [DAY, '1 day ago'],
    [3 * DAY, '3 days ago'],
    [29 * DAY + 23 * HOUR, '29 days ago'],
    [30 * DAY, 'on 18 Sep 2026'],
    [200 * DAY, 'on 1 Apr 2026'],
  ])('writes an age of %i ms as %j', (age, words) => {
    expect(timeSince(ago(age), NOW)).toBe(words)
  })

  it('says "just now" for a game from the future, which a skewed clock can make', () => {
    expect(timeSince(ago(-10 * MINUTE), NOW)).toBe('just now')
  })

  it.each(['', 'yesterday', '2026-13-45T99:99:99Z'])('does not guess at %j', (text) => {
    expect(timeSince(text, NOW)).toBe('some time ago')
  })

  it('reads the server’s format, with and without fractions', () => {
    expect(timeSince('2026-10-18T11:55:00Z', NOW)).toBe('5 minutes ago')
    expect(timeSince('2026-10-18T11:54:59.250Z', NOW)).toBe('5 minutes ago')
  })
})

describe('joinNames', () => {
  it.each([
    [[], ''],
    [['Bo'], 'Bo'],
    [['Bo', 'Cy'], 'Bo and Cy'],
    [['Bo', 'Cy', 'Di'], 'Bo, Cy and Di'],
    [['a', 'b', 'c', 'd', 'e'], 'a, b, c, d and e'],
  ])('joins %j as %j', (names, text) => {
    expect(joinNames(names)).toBe(text)
  })
})

describe('the other players', () => {
  it('shows the username of an account and the seat name of a guest', () => {
    expect(playerLabel({ seat: 1, name: 'Player 2', user: 'bo' })).toBe('bo')
    expect(playerLabel({ seat: 1, name: 'Player 2', user: null })).toBe('Player 2')
  })

  it('lists them in seat order, accounts by username', () => {
    const game = listed({
      others: [
        { seat: 1, name: 'Player 2', user: 'bo' },
        { seat: 2, name: 'Cy', user: null },
        { seat: 3, name: 'Player 4', user: 'di' },
      ],
    })
    expect(othersText(game)).toBe('bo, Cy and di')
  })
})

describe('resultText', () => {
  const others = [
    { seat: 1, name: 'Player 2', user: 'bo' },
    { seat: 2, name: 'Cy', user: null },
  ]

  it.each([
    ['the user won', { seat: 0, result: { Winner: 0 } as const }, 'You won'],
    ['the user is not seat 0 and won', { seat: 2, result: { Winner: 2 } as const }, 'You won'],
    ['an account won', { seat: 0, result: { Winner: 1 } as const }, 'bo won'],
    ['a guest won', { seat: 0, result: { Winner: 2 } as const }, 'Cy won'],
    ['it was a draw', { seat: 0, result: 'Draw' as const }, 'Draw'],
    ['the finished game has no result', { seat: 0, result: null }, 'Finished'],
    [
      'the winner is a seat the list does not know',
      { seat: 0, result: { Winner: 7 } as const },
      'Seat 8 won',
    ],
  ])('says who won when %s', (_what, over, words) => {
    expect(resultText(listed({ status: 'finished', others, ...over }))).toBe(words)
  })

  it.each(['lobby', 'running'] as const)('says nothing for a game in the %s state', (status) => {
    expect(resultText(listed({ status, others }))).toBeNull()
  })
})
