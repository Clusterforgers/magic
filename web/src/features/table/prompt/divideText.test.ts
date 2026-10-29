/** Goal: the split of a DivideAmount prompt starts at the server's even division, keeps every share from one to what the others leave, is legal exactly when the shares add up to the total, and is sent in the order of the targets. */
import { describe, expect, it } from 'vitest'
import type { Target } from '@/protocol/generated'
import { divideSentence, divisionAnswer, evenShares, judgeShares, mostFor, setShare } from './divideText'

describe('evenShares', () => {
  it.each<[number, number, number[]]>([
    [3, 1, [3]],
    [3, 2, [2, 1]],
    [4, 2, [2, 2]],
    [7, 3, [3, 2, 2]],
    [10, 4, [4, 2, 2, 2]],
    [2, 2, [1, 1]],
    [2, 3, [1, 1, 1]],
    [5, 0, []],
  ])('gives %i among %i the shares %j', (total, count, shares) => {
    expect(evenShares(total, count)).toEqual(shares)
  })

  it.each([
    [6, 3],
    [11, 4],
    [9, 9],
  ])('adds up to the total of %i among %i', (total, count) => {
    expect(evenShares(total, count).reduce((a, b) => a + b, 0)).toBe(total)
  })
})

describe('mostFor and setShare', () => {
  it.each<[string, number[], number, number, number, number[]]>([
    ['more while some is left', [1, 1, 1], 5, 0, 3, [3, 1, 1]],
    ['no more than the others leave', [1, 2], 5, 0, 9, [3, 2]],
    ['nothing more when nothing is left', [2, 2], 4, 0, 9, [2, 2]],
    ['not below one', [2, 2], 4, 1, 0, [2, 1]],
    ['not below one, negative', [2, 2], 4, 1, -3, [2, 1]],
    ['a fraction rounds', [2, 2], 5, 0, 2.6, [3, 2]],
    ['nothing typed keeps it', [2, 2], 4, 0, Number.NaN, [2, 2]],
  ])('%s', (_what, shares, total, index, wanted, expected) => {
    expect(setShare(shares, index, wanted, total)).toEqual(expected)
  })

  it('names the most a row may hold', () => {
    expect(mostFor([2, 1, 1], 0, 6)).toBe(4)
    expect(mostFor([2, 1, 1], 1, 6)).toBe(3)
    expect(mostFor([1, 1], 0, 1)).toBe(1)
  })
})

describe('judgeShares', () => {
  it.each<[string, number, number[], boolean, number, string]>([
    ['exact', 3, [2, 1], true, 0, 'All 3 given out.'],
    ['some left', 5, [2, 1], false, 2, '2 left to give.'],
    ['too many', 3, [2, 2], false, -1, '1 too many.'],
    ['one left', 4, [1, 2], false, 1, '1 left to give.'],
    ['too small a total', 2, [1, 1, 1], false, -1, 'There is not enough to give each target 1.'],
    ['no target', 3, [], false, 3, 'There is nothing to divide among.'],
  ])('%s', (_what, total, shares, legal, left, status) => {
    expect(judgeShares(total, shares)).toEqual({ legal, left, status })
  })
})

describe('the sentence and the answer', () => {
  it.each([
    [
      'Forked Bolt',
      'Divide 2 damage among the targets.',
      2,
      'Forked Bolt: Divide 2 damage among the targets. Each target gets at least 1.',
    ],
    ['Forked Bolt', '', 2, 'Forked Bolt: divide 2 among the targets. Each target gets at least 1.'],
  ])('asks as "%s" with "%s"', (asker, text, total, sentence) => {
    expect(divideSentence(asker, text, total)).toBe(sentence)
  })

  it('pairs each target with its share', () => {
    const targets: Target[] = [{ Player: 1 }, { Object: { id: 9, counter: 0 } }]
    expect(divisionAnswer(targets, [2, 1])).toEqual({
      DivideAmount: {
        Division: [
          [{ Player: 1 }, 2],
          [{ Object: { id: 9, counter: 0 } }, 1],
        ],
      },
    })
  })
})
