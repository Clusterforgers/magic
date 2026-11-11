// Goal: a turn time reads as m:ss below an hour and h:mm:ss from one, rounded down to whole seconds, and its words name each unit with the right number.
import { describe, expect, it } from 'vitest'
import { formatTurnTime, turnTimeText } from './turnTime'

describe('formatTurnTime', () => {
  it.each([
    [0, '0:00'],
    [999, '0:00'],
    [1_000, '0:01'],
    [65_000, '1:05'],
    [725_400, '12:05'],
    [3_599_999, '59:59'],
    [3_600_000, '1:00:00'],
    [3_725_000, '1:02:05'],
    [36_000_000, '10:00:00'],
    [-5, '0:00'],
  ])('%i ms reads %s', (ms, text) => {
    expect(formatTurnTime(ms)).toBe(text)
  })
})

describe('turnTimeText', () => {
  it.each([
    [0, 'Time used on own turns: 0 seconds'],
    [1_000, 'Time used on own turns: 1 second'],
    [60_000, 'Time used on own turns: 1 minute 0 seconds'],
    [725_000, 'Time used on own turns: 12 minutes 5 seconds'],
    [3_601_000, 'Time used on own turns: 1 hour 0 minutes 1 second'],
    [7_322_000, 'Time used on own turns: 2 hours 2 minutes 2 seconds'],
  ])('%i ms reads %s', (ms, text) => {
    expect(turnTimeText(ms)).toBe(text)
  })
})
