// Goal: each line of a partly automatic card is marked "Runs automatically" or "You do this by hand" from the clauses sent, a printed line with a clause is cut at it, a clause missing from the text leaves every line unmarked, and the by-hand line count adds the distinct clauses of the partial copies.
import { describe, expect, it } from 'vitest'
import type { Reason } from '@/protocol/generated'
import { linesByHand, linesText } from './byHand'
import { cardWords, lineMarks } from './lineMarks'

const reason = (start: number, end: number, text = 'why', face = 0): Reason => ({
  face,
  span: { start, end },
  text,
})

describe('lineMarks', () => {
  it.each([
    ['whole line by hand', 'Flying\nDraw a card.', [reason(7, 19)], ['auto', 'hand']],
    ['no clause: all automatic', 'Flying\nVigilance', [], ['auto', 'auto']],
    ['a line cut at its clause', 'Flying, draw a card.', [reason(8, 20)], ['auto', 'hand']],
    ['two clauses on one line', 'A and B', [reason(0, 1), reason(6, 7)], ['hand', 'auto', 'hand']],
    ['a clause past the text', 'Flying', [reason(40, 50)], ['none']],
  ] as const)('%s', (_name, text, reasons, marks) => {
    const found = lineMarks(text, [...reasons], 0).lines.map((line) => line.mark ?? 'none')
    expect(found).toEqual(marks)
  })

  it('shows a clause that is not found under the by-hand mark', () => {
    expect(lineMarks('Flying', [reason(40, 50, 'Does a thing.')], 0).unplaced).toEqual(['Does a thing.'])
  })

  it('names the card level words', () => {
    expect(cardWords('forge')).toBe('Runs automatically')
    expect(cardWords('partial')).toBe('Partly automatic')
    expect(cardWords('manual')).toBe('You do this by hand')
  })
})

describe('linesByHand', () => {
  it('counts the distinct clauses of the partial copies only', () => {
    const cards = [
      { support: 'partial', count: 2, reasons: ['a', 'a', 'b'] },
      { support: 'manual', count: 3, reasons: ['x'] },
      { support: 'verified', count: 1, reasons: [] },
    ] as const
    expect(linesByHand(cards)).toBe(4)
    expect(linesText(1)).toBe('1 line by hand')
    expect(linesText(4)).toBe('4 lines by hand')
  })
})
