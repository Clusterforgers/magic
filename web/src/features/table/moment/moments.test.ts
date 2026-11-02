// Goal: a batch's public reveals and rolls become moments for every seat (a look or a hidden event never does), a reveal is sized to the window, and a roll's faces end on the result.
import { describe, expect, it } from 'vitest'
import type { LogEntry } from '@/play/store'
import type { ObjectView } from '@/protocol/generated'
import { buildFixture } from '../dev/fixture'
import { dieShape, diePoints, MOST_MOMENTS, momentsOf, revealWidth, rollFaces, withMoments } from './moments'
import type { Moment } from './moments'

const card = buildFixture({ seats: 2 }).view.objects[0] as ObjectView
const entry = (seq: number, event: LogEntry['event'], seen?: readonly ObjectView[]): LogEntry =>
  seen === undefined ? { seq, event } : { seq, event, seen }
const nameOf = (): string | undefined => undefined

describe('momentsOf', () => {
  it.each([
    [
      'a public reveal with its cards',
      entry(1, { Public: { Revealed: { player: 1, cards: [card.id] } } }, [card]),
      'reveal',
    ],
    ['a die roll', entry(2, { Public: { DieRolled: { player: 1, sides: 20, result: 7 } } }), 'roll'],
    [
      'a die rolled by hand',
      entry(3, { Public: { ManualRolled: { player: 0, kind: { Die: { sides: 6 } }, result: 4 } } }),
      'roll',
    ],
  ] as const)('%s makes one %s moment', (_, logged, kind) => {
    const moments = momentsOf({ entries: [logged], nameOf })
    expect(moments.map((m) => m.kind)).toEqual([kind])
  })

  it.each([
    [
      'a reveal whose cards the view never held',
      entry(1, { Public: { Revealed: { player: 1, cards: [9] } } }),
    ],
    [
      'a look at the library',
      entry(2, { Public: { LibraryLooked: { player: 0, cards: [card.id] } } }, [card]),
    ],
    [
      'a roll of a die with no faces',
      entry(3, { Public: { DieRolled: { player: 0, sides: 0, result: 0 } } }),
    ],
  ])('%s makes none', (_, logged) => {
    expect(momentsOf({ entries: [logged], nameOf })).toEqual([])
  })

  it('keeps who rolled and the result', () => {
    const [moment] = momentsOf({
      entries: [entry(5, { Public: { DieRolled: { player: 1, sides: 20, result: 13 } } })],
      nameOf,
    })
    expect(moment).toMatchObject({ kind: 'roll', player: 1, sides: 20, result: 13, key: '5' })
  })
})

describe('withMoments', () => {
  it('keeps the newest few', () => {
    const roll = (n: number): Moment => ({
      kind: 'roll',
      key: `${n}`,
      player: 0,
      sides: 6,
      result: 1,
      why: null,
    })
    const kept = withMoments([roll(1), roll(2), roll(3)], [roll(4), roll(5)])
    expect(kept.map((m) => m.key)).toEqual(['3', '4', '5'])
    expect(kept).toHaveLength(MOST_MOMENTS)
  })
})

describe('revealWidth', () => {
  it.each([
    [1, { width: 1400, height: 900 }, 300, false],
    [3, { width: 1024, height: 768 }, 219, true],
    [3, { width: 1024, height: 768 }, 274, false],
    [1, { width: 1400, height: 500 }, 178, false],
    [8, { width: 1400, height: 900 }, 157, false],
    [3, { width: 390, height: 700 }, 103, false],
    [30, { width: 390, height: 700 }, 96, false],
  ])('%i cards in %j are %i px wide', (count, window, expected, crowded) => {
    expect(revealWidth(count, window, crowded)).toBe(expected)
  })
})

describe('rollFaces', () => {
  it.each([
    [20, 20],
    [6, 1],
    [2, 2],
    [1, 1],
    [100, 57],
  ])('a d%i ends on %i with faces on the die, and a big die never repeats a face', (sides, result) => {
    const faces = rollFaces(sides, result, 'k')
    expect(faces.at(-1)).toBe(result)
    expect(faces.every((f) => f >= 1 && f <= sides)).toBe(true)
    if (sides > 20) faces.slice(1).forEach((f, i) => expect(f).not.toBe(faces[i]))
  })

  it('shows every seat the same faces', () => {
    expect(rollFaces(20, 3, 'same')).toEqual(rollFaces(20, 3, 'same'))
  })
})

describe('dieShape', () => {
  it.each([
    [4, 'triangle'],
    [6, 'square'],
    [20, 'hexagon'],
    [7, 'circle'],
  ] as const)('a d%i is a %s', (sides, shape) => {
    expect(dieShape(sides)).toBe(shape)
    expect(diePoints(shape) === null).toBe(shape === 'circle')
  })
})
