/** Goal: every kind of stack-forecast change reads in the fewest words ("+3/+3 → 7/7", "3 damage · dies", "−3 → 37"), signs carry a true minus, and a card or seat with no change has no words. */
import { describe, expect, it } from 'vitest'
import type { Change } from '@/protocol/generated'
import { changeWords, forecastWords, signed } from './forecastWords'

describe('one change', () => {
  it.each<[string, Change, string]>([
    ['a pump', { Stats: { from: [4, 4], to: [7, 7] } }, '+3/+3 → 7/7'],
    ['a shrink', { Stats: { from: [3, 3], to: [1, 1] } }, '−2/−2 → 1/1'],
    ['a power-only change', { Stats: { from: [2, 2], to: [4, 2] } }, '+2/+0 → 4/2'],
    ['damage', { Damage: { amount: 3 } }, '3 damage'],
    ['destroyed', { Leaves: { fate: 'Destroyed' } }, 'destroyed'],
    ['a death', { Leaves: { fate: 'Dies' } }, 'dies'],
    ['sacrificed', { Leaves: { fate: 'Sacrificed' } }, 'sacrificed'],
    ['exiled', { Leaves: { fate: 'Exiled' } }, 'exiled'],
    ['a bounce', { Leaves: { fate: 'Hand' } }, 'to hand'],
    ['to the library', { Leaves: { fate: 'Library' } }, 'to library'],
    ['to the graveyard', { Leaves: { fate: 'Graveyard' } }, 'to graveyard'],
    ['countered', { Leaves: { fate: 'Countered' } }, 'countered'],
    ['tapped', 'Tapped', 'tapped'],
    ['untapped', 'Untapped', 'untapped'],
    ['two counters', { Counters: { kind: '+1/+1', from: 0, to: 2 } }, '+2 +1/+1 counters'],
    ['one loyalty counter off', { Counters: { kind: 'loyalty', from: 4, to: 3 } }, '−1 loyalty counter'],
    ['a named counter', { Counters: { kind: { Other: 'lore' }, from: 1, to: 2 } }, '+1 lore counter'],
    ['life lost', { Life: { from: 40, to: 37 } }, '−3 → 37'],
    ['life gained', { Life: { from: 20, to: 24 } }, '+4 → 24'],
    ['life to zero', { Life: { from: 3, to: 0 } }, '−3 → 0'],
  ])('%s reads "%s"', (_what, change, words) => {
    expect(changeWords(change)).toBe(words)
  })

  it('names the new controller when it can', () => {
    const change: Change = { Control: { to: 1 } }
    expect(changeWords(change)).toBe('changes control')
    expect(changeWords(change, (seat) => `Seat ${seat + 1}`)).toBe('to Seat 2')
  })
})

describe('a card or seat', () => {
  it('joins its changes in the engine order', () => {
    expect(forecastWords([{ Damage: { amount: 3 } }, { Leaves: { fate: 'Dies' } }])).toBe('3 damage · dies')
  })

  it('has no words without a change', () => {
    expect(forecastWords([])).toBeNull()
  })

  it.each([
    [3, '+3'],
    [0, '+0'],
    [-4, '−4'],
  ])('signs %i as %s', (delta, words) => {
    expect(signed(delta)).toBe(words)
  })
})
