/** Goal: the words of loops: card lists, what an iteration or a shortcut changed (tokens per controller, life up and down), the log line of a shortcut with and without a chooser, the draw line, the checks of a typed number of iterations, a batch end that counts folded events, and a thousand identical loop tokens drawn as one stack. */
import { describe, expect, it } from 'vitest'
import type { EventBatch, LoopSummary } from '@/protocol/generated'
import { card } from '@/test-utils/cards'
import { batchEnd } from '@/play/view'
import { groupPermanents } from './group'
import { iterationsProblem, loopCards, loopDrawLine, loopSummaryLine, stepWords } from './loopText'

const seat = (who: number): string => ['Ann', 'Bob', 'Cid'][who] ?? `Seat ${who}`

describe('loopCards', () => {
  it.each<[string[], string]>([
    [[], 'A loop'],
    [['Polyraptor'], 'Polyraptor'],
    [['Polyraptor', 'Aether Flash'], 'Polyraptor and Aether Flash'],
    [['A', 'B', 'C'], 'A, B and C'],
  ])('%j reads %s', (cards, words) => expect(loopCards(cards)).toBe(words))
})

describe('stepWords', () => {
  it('names tokens per controller and life both ways', () => {
    expect(
      stepWords(
        {
          tokens: [{ controller: 0, name: 'Pestermite', count: 2 }],
          life: [
            { player: 0, change: 3 },
            { player: 1, change: -3 },
          ],
        },
        seat,
      ),
    ).toEqual(['+2 Pestermite tokens for Ann', 'Ann gains 3 life', 'Bob loses 3 life'])
  })

  it('is empty for a loop that changes nothing it counts', () => expect(stepWords({}, seat)).toEqual([]))
})

describe('log lines', () => {
  const summary: LoopSummary = {
    player: 0,
    cards: ['Polyraptor', 'Where Ancients Tread'],
    iterations: 100,
    total: { tokens: [{ controller: 0, name: 'Polyraptor', count: 100 }] },
    events: 2300,
  }

  it('sums up a shortcut', () => {
    expect(loopSummaryLine(summary, seat)).toBe(
      'Polyraptor and Where Ancients Tread loop repeated 100 times by Ann: +100 Polyraptor tokens for Ann',
    )
  })

  it('leaves out the chooser of a loop that ran by itself', () => {
    const { player: _, ...ran } = summary
    expect(loopSummaryLine({ ...ran, iterations: 1, total: {} }, seat)).toBe(
      'Polyraptor and Where Ancients Tread loop repeated 1 time',
    )
  })

  it('says why the game is a draw', () => {
    expect(loopDrawLine(['Polyraptor', 'Aether Flash'])).toBe(
      'Polyraptor and Aether Flash would loop forever with no one able to stop it: the game is a draw',
    )
  })
})

describe('iterationsProblem', () => {
  it.each<[string, string | null]>([
    ['5', null],
    ['10000', null],
    ['0', 'At least 1'],
    ['10001', 'At most 10,000'],
    ['2.5', 'Enter a whole number'],
    ['x', 'Enter a whole number'],
  ])('%s', (typed, problem) => expect(iterationsProblem(typed, 10_000)).toBe(problem))
})

describe('a loop batch', () => {
  it('ends after its folded events', () => {
    const batch = { from_seq: 40, events: [], folded: 2300 } as unknown as EventBatch
    expect(batchEnd(batch)).toBe(2340)
    expect(batchEnd({ ...batch, folded: undefined })).toBe(40)
  })

  it('draws a thousand identical tokens as one stack', () => {
    const tokens = Array.from({ length: 1000 }, (_, at) =>
      card({ id: 100 + at, name: 'Polyraptor', kind: 'Token' }),
    )
    const stacks = groupPermanents(tokens)
    expect(stacks).toHaveLength(1)
    expect(stacks[0]?.count).toBe(1000)
  })
})
