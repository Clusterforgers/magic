// Goal: the marks of a batch become the badges to show and the arrows to draw: only the eight kinds with a face are shown, each kind once on an object, at most `MAX_MARKS`; only a counter, an exile, a bounce or a death caused by a resolving object points an arrow from it, each pair once, never from an object to itself, at most `MAX_MARK_ARROWS`.
import { describe, expect, it } from 'vitest'
import { MARK_FX, FX_MS } from './model'
import { markArrows, markEffects, MARK_WORDS, MAX_MARK_ARROWS, MAX_MARKS } from './marks'
import { MOVE_MS } from './moves'
import type { MarkKind, MoveMark } from './moves'

const mark = (kind: MarkKind, object: number, by?: number): MoveMark =>
  by === undefined ? { kind, object } : { kind, object, by }

describe('which marks are shown', () => {
  it.each(MARK_FX)('shows %s on its object with a word of its own', (kind) => {
    expect(markEffects([mark(kind, 7)])).toEqual([{ kind, object: 7 }])
    expect(MARK_WORDS[kind]).toMatch(/^[A-Z][a-z]+$/)
    expect(FX_MS[kind]).toBe(MOVE_MS.mark)
  })

  it.each<MarkKind>(['targeted', 'died'])('leaves %s to the arrows and the ghosts', (kind) => {
    expect(markEffects([mark(kind, 7, 3)])).toEqual([])
  })

  it('keeps each kind once on an object, and the same kind on two objects', () => {
    const shown = markEffects([
      mark('resolved', 1),
      mark('resolved', 1),
      mark('resolved', 2),
      mark('exiled', 1, 9),
      mark('exiled', 1),
    ])
    expect(shown).toEqual([
      { kind: 'resolved', object: 1 },
      { kind: 'resolved', object: 2 },
      { kind: 'exiled', object: 1 },
    ])
  })

  it('stops at the cap, keeping the first marks', () => {
    const many = Array.from({ length: MAX_MARKS + 5 }, (_, object) => mark('drawn', object))
    const shown = markEffects(many)
    expect(shown).toHaveLength(MAX_MARKS)
    expect(shown[0]).toEqual({ kind: 'drawn', object: 0 })
  })
})

describe('which marks point an arrow', () => {
  it.each<[MarkKind, boolean]>([
    ['countered', true],
    ['exiled', true],
    ['bounced', true],
    ['died', true],
    ['resolved', false],
    ['fizzled', false],
    ['drawn', false],
    ['activated', false],
    ['triggered', false],
    ['targeted', false],
  ])('%s: %s', (kind, points) => {
    expect(markArrows([mark(kind, 7, 3)])).toEqual(points ? [{ by: 3, object: 7, kind }] : [])
  })

  it('needs a resolving object that is another object', () => {
    expect(markArrows([mark('exiled', 7), mark('exiled', 8, 8)])).toEqual([])
  })

  it('draws a pair once, whatever the marks that name it', () => {
    expect(markArrows([mark('died', 7, 3), mark('exiled', 7, 3), mark('died', 8, 3)])).toEqual([
      { by: 3, object: 7, kind: 'died' },
      { by: 3, object: 8, kind: 'died' },
    ])
  })

  it('stops at the cap', () => {
    const wipe = Array.from({ length: MAX_MARK_ARROWS + 3 }, (_, object) => mark('died', object + 10, 3))
    expect(markArrows(wipe)).toHaveLength(MAX_MARK_ARROWS)
  })
})
