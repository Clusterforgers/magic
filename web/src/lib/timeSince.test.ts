// Goal: an age is worded right at every boundary of minute, hour, day and month, never guessed from text that is no date, and a clock that is ahead of the server never gives a negative age.
import { describe, expect, it } from 'vitest'
import { timeSince } from './timeSince'

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
