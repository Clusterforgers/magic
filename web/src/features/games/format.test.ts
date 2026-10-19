// Goal: the words of the games list are right for every age, every set of other players and every way a game can end, whoever won and however many seats there were.
import { describe, expect, it } from 'vitest'
import { listed } from '@/test-utils/accountsServer'
import { joinNames, othersText, playerLabel, resultText } from './format'

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
