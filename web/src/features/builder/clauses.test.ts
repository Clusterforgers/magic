// Goal: the oracle text of a face is cut at the spans of its unsupported clauses into stretches that join back into the text and say which clauses cover each and which end there: overlapping spans share a stretch, adjacent spans stay apart, a span past the end is cut back or ignored, a clause on another face covers nothing here and keeps its place in the card's numbering, spans count Unicode scalar values, and an empty text has no stretch.
import { describe, expect, it } from 'vitest'
import type { Reason } from '@/protocol/generated'
import { segmentsOf } from './clauses'
import type { Segment } from './clauses'

const clause = (start: number, end: number, face = 0): Reason => ({
  face,
  span: { start, end },
  text: `clause ${start}-${end}`,
})
const plain = (text: string): Segment => ({ text, covers: [], ends: [] })
const seg = (text: string, covers: number[], ends: number[]): Segment => ({ text, covers, ends })

// Positions: a0 b1 c2 d3 e4 f5 g6 h7 i8 j9.
const TEXT = 'abcdefghij'

describe('segmentsOf', () => {
  it.each([
    ['an empty text has no stretch', '', [clause(0, 5)], 0, []],
    ['a text with no clause is one plain stretch', TEXT, [], 0, [plain(TEXT)]],
    [
      'a span in the middle cuts three stretches',
      TEXT,
      [clause(2, 5)],
      0,
      [plain('ab'), seg('cde', [0], [0]), plain('fghij')],
    ],
    ['a span at the start', TEXT, [clause(0, 3)], 0, [seg('abc', [0], [0]), plain('defghij')]],
    ['a span at the end', TEXT, [clause(7, 10)], 0, [plain('abcdefg'), seg('hij', [0], [0])]],
    ['a span over the whole text', TEXT, [clause(0, 10)], 0, [seg(TEXT, [0], [0])]],
    [
      'adjacent spans stay two stretches, each with its own clause',
      TEXT,
      [clause(0, 4), clause(4, 10)],
      0,
      [seg('abcd', [0], [0]), seg('efghij', [1], [1])],
    ],
    [
      'overlapping spans share the stretch they have in common',
      TEXT,
      [clause(2, 6), clause(4, 8)],
      0,
      [plain('ab'), seg('cd', [0], []), seg('ef', [0, 1], [0]), seg('gh', [1], [1]), plain('ij')],
    ],
    [
      'a span inside another ends its own marker before the outer one',
      TEXT,
      [clause(1, 9), clause(3, 5)],
      0,
      [plain('a'), seg('bc', [0], []), seg('de', [0, 1], [1]), seg('fghi', [0], [0]), plain('j')],
    ],
    [
      'two clauses on one span share the stretch and both end there',
      TEXT,
      [clause(2, 5), clause(2, 5)],
      0,
      [plain('ab'), seg('cde', [0, 1], [0, 1]), plain('fghij')],
    ],
    [
      'a span that runs past the end is cut back to the text',
      TEXT,
      [clause(7, 50)],
      0,
      [plain('abcdefg'), seg('hij', [0], [0])],
    ],
    ['a span wholly past the end covers nothing', TEXT, [clause(20, 30)], 0, [plain(TEXT)]],
    [
      'a span that starts before the text is cut back to it',
      TEXT,
      [clause(-5, 3)],
      0,
      [seg('abc', [0], [0]), plain('defghij')],
    ],
    ['an empty span covers nothing', TEXT, [clause(4, 4)], 0, [plain(TEXT)]],
    ['a reversed span covers nothing', TEXT, [clause(6, 2)], 0, [plain(TEXT)]],
    [
      'a fractional span is cut to whole positions',
      TEXT,
      [clause(2.7, 5.2)],
      0,
      [plain('ab'), seg('cde', [0], [0]), plain('fghij')],
    ],
    ['a clause of the back face covers nothing on the front', TEXT, [clause(2, 5, 1)], 0, [plain(TEXT)]],
    [
      'a clause of the back face covers the back text, numbered among all the card clauses',
      TEXT,
      [clause(0, 2, 0), clause(2, 5, 1)],
      1,
      [plain('ab'), seg('cde', [1], [1]), plain('fghij')],
    ],
    [
      'the front keeps its own clause when the back has one too',
      TEXT,
      [clause(0, 2, 0), clause(2, 5, 1)],
      0,
      [seg('ab', [0], [0]), plain('cdefghij')],
    ],
    ['a face that has no clause is one plain stretch', TEXT, [clause(2, 5, 0)], 2, [plain(TEXT)]],
    [
      'spans count characters outside the Basic Multilingual Plane as one',
      'a😀b😀c',
      [clause(1, 3)],
      0,
      [plain('a'), seg('😀b', [0], [0]), plain('😀c')],
    ],
    [
      'a clause may start after a line break',
      'Flying\nHaste',
      [clause(7, 12)],
      0,
      [plain('Flying\n'), seg('Haste', [0], [0])],
    ],
  ] as const satisfies ReadonlyArray<
    readonly [string, string, readonly Reason[], number, readonly Segment[]]
  >)('%s', (_what, text, reasons, face, expected) => {
    expect(segmentsOf(text, reasons, face)).toEqual(expected)
  })
})

// A small seeded generator, so a failure repeats.
function lcg(seed: number): () => number {
  let state = seed
  return () => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0
    return state / 2 ** 32
  }
}

describe('segmentsOf over random texts and spans', () => {
  const ALPHABET = ['a', 'b', ' ', '\n', '—', '😀', '𝔸']
  const cases = Array.from({ length: 300 }, (_, seed) => seed)

  it.each(cases)('keeps its promises for case %i', (seed) => {
    const next = lcg(seed + 1)
    const pick = (below: number): number => Math.floor(next() * below)
    const chars = Array.from({ length: pick(21) }, () => ALPHABET[pick(ALPHABET.length)] ?? 'a')
    const text = chars.join('')
    const reasons = Array.from({ length: pick(5) }, () => clause(pick(30) - 4, pick(30) - 4, pick(2)))
    const face = pick(2)
    const segments = segmentsOf(text, reasons, face)

    expect(segments.map((entry) => entry.text).join('')).toBe(text)
    expect(segments.every((entry) => entry.text !== '')).toBe(true)

    let position = 0
    const ended = new Map<number, number>()
    for (const entry of segments) {
      const length = Array.from(entry.text).length
      for (let at = position; at < position + length; at += 1) {
        const expected = reasons.flatMap((reason, index) =>
          reason.face === face && reason.span.start <= at && at < reason.span.end ? [index] : [],
        )
        expect(entry.covers, `position ${at}`).toEqual(expected)
      }
      position += length
      for (const index of entry.ends) ended.set(index, (ended.get(index) ?? 0) + 1)
    }
    // Every clause that covers something ends exactly one stretch.
    const covering = reasons.flatMap((reason, index) =>
      reason.face === face && segments.some((entry) => entry.covers.includes(index)) ? [index] : [],
    )
    expect([...ended.keys()].sort()).toEqual(covering)
    expect([...ended.values()].every((times) => times === 1)).toBe(true)
  })
})
