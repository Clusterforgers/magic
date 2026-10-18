// Goal: line geometry turns two boxes into a curve and an arrowhead that start and stop a fixed gap outside the boxes, on the side that faces the other box, for every placement of the two (beside, above, diagonal, a card to a wide panel), bows to either side, mirrors when the boxes swap, and draws nothing when there is no room (boxes not laid out, concentric, overlapping or closer than the gap).
import { describe, expect, it } from 'vitest'
import { centreOf, connect, exitPoint, isLaidOut } from './geometry'
import type { Point, Rect } from './geometry'

const box = (x: number, y: number, width = 100, height = 60): Rect => ({ x, y, width, height })

/** Distance from a point to a box; 0 inside it. */
function distanceTo(rect: Rect, point: Point): number {
  const dx = Math.max(rect.x - point.x, 0, point.x - (rect.x + rect.width))
  const dy = Math.max(rect.y - point.y, 0, point.y - (rect.y + rect.height))
  return Math.hypot(dx, dy)
}

/** The numbers in a path, in order. */
const numbers = (path: string): number[] => (path.match(/-?\d+(\.\d+)?/g) ?? []).map(Number)

describe('exitPoint', () => {
  const rect = box(0, 0, 100, 60)

  it.each<[string, Point, Point]>([
    ['below', { x: 50, y: 1000 }, { x: 50, y: 63 }],
    ['above', { x: 50, y: -1000 }, { x: 50, y: -3 }],
    ['right', { x: 1000, y: 30 }, { x: 103, y: 30 }],
    ['left', { x: -1000, y: 30 }, { x: -3, y: 30 }],
  ])('leaves through the edge that faces a point %s, the gap further out', (_side, toward, expected) => {
    const point = exitPoint(rect, toward, 3)
    expect(point?.x).toBeCloseTo(expected.x, 9)
    expect(point?.y).toBeCloseTo(expected.y, 9)
  })

  it('leaves a corner when the line points at it, the gap along the line', () => {
    const point = exitPoint(rect, { x: 150, y: 90 }, 3)
    expect(point?.x).toBeCloseTo(102.57, 2)
    expect(point?.y).toBeCloseTo(61.54, 2)
  })

  it.each<[string, Point]>([
    ['the same point', { x: 50, y: 30 }],
    ['a point inside the box', { x: 80, y: 40 }],
    ['a point on its edge', { x: 100, y: 30 }],
  ])('has no exit toward %s', (_what, toward) => {
    expect(exitPoint(rect, toward, 3)).toBeNull()
  })
})

describe('isLaidOut', () => {
  it.each<[string, Rect, boolean]>([
    ['a box with room', box(0, 0), true],
    ['a box away from the origin', box(-40, 900), true],
    ['a box with no width', box(0, 0, 0, 60), false],
    ['a box with no height', box(0, 0, 100, 0), false],
    ['the all-zero box of a hidden element', { x: 0, y: 0, width: 0, height: 0 }, false],
    ['a negative size', box(0, 0, -5, 60), false],
    ['not a number', box(Number.NaN, 0), false],
    ['infinite', box(0, 0, Number.POSITIVE_INFINITY, 60), false],
  ])('%s', (_what, rect, expected) => {
    expect(isLaidOut(rect)).toBe(expected)
  })
})

const PLACEMENTS: [string, Rect, Rect][] = [
  ['beside, to the right', box(0, 0), box(300, 0)],
  ['beside, to the left', box(300, 0), box(0, 0)],
  ['below', box(0, 0), box(0, 300)],
  ['above', box(0, 300), box(0, 0)],
  ['down and to the right', box(0, 0), box(260, 240)],
  ['up and to the left', box(260, 240), box(0, 0)],
  ['up and to the right', box(0, 240), box(260, 0)],
  ['down and to the left', box(260, 0), box(0, 240)],
  ['a card to a wide panel below it', box(120, 0, 96, 134), box(0, 400, 800, 260)],
  ['a card to a wide panel below and to its side', box(700, 0, 96, 134), box(0, 400, 400, 260)],
  ['a card to a panel above it', box(300, 700, 96, 134), box(0, 0, 800, 260)],
  ['a tapped card to a tile', box(10, 10, 134, 96), box(400, 20, 160, 70)],
  ['just enough room', box(0, 0), box(120, 0)],
]

describe('connect', () => {
  describe.each(PLACEMENTS)('%s', (_name, from, to) => {
    const shape = connect(from, to)

    it('draws a curve and a head made of plain numbers', () => {
      expect(shape).not.toBeNull()
      expect(shape?.d).toMatch(/^M -?[\d.]+ -?[\d.]+ Q (-?[\d.]+ ){3}-?[\d.]+$/)
      expect(shape?.head).toMatch(/^M (-?[\d.]+ ){2}L (-?[\d.]+ ){2}L (-?[\d.]+ ){2}Z$/)
    })

    it('starts and stops outside the boxes, within the gap of each', () => {
      expect(shape).not.toBeNull()
      if (shape === null) return
      expect(distanceTo(from, shape.start)).toBeGreaterThan(0)
      expect(distanceTo(from, shape.start)).toBeLessThanOrEqual(3 + 1e-9)
      expect(distanceTo(to, shape.end)).toBeGreaterThan(0)
      expect(distanceTo(to, shape.end)).toBeLessThanOrEqual(3 + 1e-9)
    })

    it('runs toward the other box, not away from it', () => {
      if (shape === null) return
      const a = centreOf(from)
      const b = centreOf(to)
      const along = (shape.end.x - shape.start.x) * (b.x - a.x) + (shape.end.y - shape.start.y) * (b.y - a.y)
      expect(along).toBeGreaterThan(0)
    })

    it('bows no further than its share of the distance, and its middle lies between the ends', () => {
      if (shape === null) return
      const length = Math.hypot(shape.end.x - shape.start.x, shape.end.y - shape.start.y)
      const middle = { x: (shape.start.x + shape.end.x) / 2, y: (shape.start.y + shape.end.y) / 2 }
      expect(Math.hypot(shape.mid.x - middle.x, shape.mid.y - middle.y)).toBeCloseTo(0.5 * 0.12 * length, 6)
    })

    it('mirrors when the boxes swap: each end is where the other began', () => {
      const back = connect(to, from)
      expect(back?.start).toEqual(shape?.end)
      expect(back?.end).toEqual(shape?.start)
    })

    it('ends the curve inside the arrowhead, a little short of its tip', () => {
      if (shape === null) return
      const [, , , , x, y] = numbers(shape.d)
      const toTip = Math.hypot((x ?? 0) - shape.end.x, (y ?? 0) - shape.end.y)
      expect(toTip).toBeGreaterThan(0)
      expect(toTip).toBeLessThanOrEqual(0.7 * 9 + 0.2)
    })
  })

  describe('exact drawings (a straight line, a head of 10)', () => {
    const plain = { bow: 0, head: 10 }

    it('to the right', () => {
      const shape = connect(box(0, 0, 100, 50), box(300, 0, 100, 50), plain)
      expect(shape?.d).toBe('M 103 25 Q 196.5 25 290 25')
      expect(shape?.head).toBe('M 297 25 L 288.2 29.7 L 288.2 20.3 Z')
      expect(shape?.start).toEqual({ x: 103, y: 25 })
      expect(shape?.end).toEqual({ x: 297, y: 25 })
      expect(shape?.mid).toEqual({ x: 200, y: 25 })
    })

    it('downward', () => {
      const shape = connect(box(0, 0, 100, 50), box(0, 300, 100, 50), plain)
      expect(shape?.d).toBe('M 50 53 Q 50 171.5 50 290')
      expect(shape?.head).toBe('M 50 297 L 45.3 288.2 L 54.7 288.2 Z')
    })

    it('to the left, with the head turned around', () => {
      const shape = connect(box(300, 0, 100, 50), box(0, 0, 100, 50), plain)
      expect(shape?.d).toBe('M 297 25 Q 203.5 25 110 25')
      expect(shape?.head).toBe('M 103 25 L 111.8 20.3 L 111.8 29.7 Z')
    })
  })

  describe('the bow', () => {
    const left = box(0, 0, 100, 50)
    const right = box(300, 0, 100, 50)
    /** Where the control point of the curve is. */
    const control = (bow: number): Point => {
      const [, , x, y] = numbers(connect(left, right, { bow, head: 10 })?.d ?? '')
      return { x: x ?? Number.NaN, y: y ?? Number.NaN }
    }

    it('bends to one side for a positive share and to the other for a negative one', () => {
      expect(control(0.2).y).toBeGreaterThan(25)
      expect(control(-0.2).y).toBeLessThan(25)
      expect(control(0.2).y - 25).toBeCloseTo(25 - control(-0.2).y, 1)
    })

    it('goes straight for none', () => {
      expect(control(0).y).toBe(25)
    })

    it('bends more for a bigger share', () => {
      expect(control(0.3).y - 25).toBeGreaterThan(control(0.1).y - 25)
    })
  })

  describe('the gap and the head', () => {
    it('keeps the line the given gap away', () => {
      const wide = connect(box(0, 0), box(300, 0), { gap: 10 })
      expect(wide?.start.x).toBe(110)
      expect(wide?.end.x).toBe(290)
    })

    it('makes a longer head for a longer length, with its tip on the end', () => {
      const small = connect(box(0, 0), box(300, 0), { head: 6 })
      const large = connect(box(0, 0), box(300, 0), { head: 14 })
      const tip = (head: string | undefined): number[] => numbers(head ?? '').slice(0, 2)
      expect(tip(small?.head)).toEqual(tip(large?.head))
      const base = (head: string | undefined): number => numbers(head ?? '')[2] ?? Number.NaN
      expect(base(small?.head)).toBeGreaterThan(base(large?.head))
    })
  })

  describe.each<[string, Rect, Rect]>([
    ['the first box has not been laid out', { x: 0, y: 0, width: 0, height: 0 }, box(300, 0)],
    ['the second box has not been laid out', box(0, 0), { x: 0, y: 0, width: 0, height: 0 }],
    ['a box is not a number', box(Number.NaN, 0), box(300, 0)],
    ['the boxes are one and the same', box(0, 0), box(0, 0)],
    ['one box lies inside the other', box(0, 0, 400, 400), box(100, 100)],
    ['the boxes overlap', box(0, 0), box(40, 0)],
    ['a centre sits on the edge of the other box', box(0, 0), box(50, 0)],
    ['the boxes touch', box(0, 0), box(100, 0)],
    ['the boxes are closer than the gaps', box(0, 0), box(104, 0)],
  ])('has nothing to draw when %s', (_what, from, to) => {
    it('gives null', () => {
      expect(connect(from, to)).toBeNull()
    })
  })
})
