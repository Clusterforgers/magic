// Goal: the countdown says what happens at zero for each kind of decision that has a clock, counts whole seconds up to the next one, and the sentence for a screen reader names the right number and the right consequence.
import { describe, expect, it } from 'vitest'
import { clockAnnouncement, clockText, secondsLeft } from './clockText'

describe('clockText', () => {
  it.each([
    ['Priority', 10, 'Passing for you in 10 s'],
    ['Priority', 1, 'Passing for you in 1 s'],
    ['DeclareBlockers', 45, 'Declaring no blocks in 45 s'],
    ['ChooseTargets', 10, 'Choosing no targets in 10 s'],
    ['OrderTriggers', 3, 'Ordering the triggers for you in 3 s'],
  ] as const)('%s with %i s left reads "%s"', (kind, seconds, text) => {
    expect(clockText(kind, seconds)).toBe(text)
  })

  it('does not claim a consequence for a kind that has no clock', () => {
    expect(clockText('Discard', 5)).toBe('The game goes on without you in 5 s')
  })
})

describe('clockAnnouncement', () => {
  it.each([
    ['Priority', 10, 'You have 10 seconds to answer; after that the game passes for you.'],
    ['Priority', 1, 'You have 1 second to answer; after that the game passes for you.'],
    ['DeclareBlockers', 45, 'You have 45 seconds to answer; after that the game declares no blocks for you.'],
    ['ChooseTargets', 10, 'You have 10 seconds to answer; after that the game chooses no targets for you.'],
    ['OrderTriggers', 10, 'You have 10 seconds to answer; after that the game orders the triggers for you.'],
  ] as const)('%s with %i s reads "%s"', (kind, seconds, text) => {
    expect(clockAnnouncement(kind, seconds)).toBe(text)
  })
})

describe('secondsLeft', () => {
  it.each([
    [10_000, 10],
    [9_999, 10],
    [9_000, 9],
    [8_999.5, 9],
    [1_000, 1],
    [1, 1],
    [0, 0],
    [-5, 0],
  ])('%f ms is %i s', (ms, seconds) => {
    expect(secondsLeft(ms)).toBe(seconds)
  })
})
