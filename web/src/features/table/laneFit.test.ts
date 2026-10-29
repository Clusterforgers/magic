// Goal: every permanent lands in its lane by type, and the lane fit picks the largest card of the ladder at which every lane of a board fits its box without overlap, keeps the card shape, takes no room for an empty lane, mirrors an opponent's bands and grows with the room.
import { describe, expect, it } from 'vitest'
import type { CardType, ObjectView, Subtype } from '@/protocol/generated'
import { card } from '@/test-utils/cards'
import { rowOf } from './cardModel'
import type { RowKind } from './cardModel'
import { CARD_ASPECT, LAND_SCALE } from './fit'
import type { FitBox } from './fit'
import { fitLanes, LANE_LIMITS } from './laneFit'
import type { LaneCounts, LaneFit, LaneLimits } from './laneFit'

const typed = (types: CardType[], subtypes: Subtype[] = []): ObjectView => card({ id: 1, types, subtypes })

describe('the lane of a permanent', () => {
  it.each<[string, ObjectView, RowKind]>([
    ['a creature', typed(['Creature']), 'creatures'],
    ['an artifact creature', typed(['Artifact', 'Creature']), 'creatures'],
    ['a land creature', typed(['Land', 'Creature']), 'creatures'],
    ['an enchantment creature', typed(['Enchantment', 'Creature']), 'creatures'],
    ['a land', typed(['Land']), 'lands'],
    ['an artifact land', typed(['Artifact', 'Land']), 'lands'],
    ['an artifact', typed(['Artifact']), 'other'],
    ['an enchantment', typed(['Enchantment']), 'other'],
    ['a Background', typed(['Enchantment'], ['Background']), 'other'],
    ['a planeswalker', typed(['Planeswalker']), 'other'],
    ['a battle', typed(['Battle']), 'other'],
  ])('puts %s in the %s lane', (_name, object, lane) => {
    expect(rowOf(object)).toBe(lane)
  })
})

const counts = (creatures: number, other: number, lands: number): LaneCounts => ({ creatures, other, lands })
const ladder = (limits: LaneLimits = LANE_LIMITS): number[] =>
  Array.from(
    { length: Math.floor((limits.maxWidth - limits.minWidth) / limits.step) + 1 },
    (_, rung) => limits.minWidth + rung * limits.step,
  )
const lanesOf = (fit: LaneFit) => fit.bands.flatMap((band) => band.lanes)

/** Checks the fit against the box: every band inside the width, all bands inside the height, every card 5:7. */
function holds(fit: LaneFit, box: FitBox, label: string): void {
  const { gap, rowGap, laneGap, bandGap } = LANE_LIMITS
  for (const band of fit.bands) {
    const span = band.lanes.reduce((sum, lane) => sum + lane.span, 0) + laneGap * (band.lanes.length - 1)
    expect(span, `${label}: band width`).toBeLessThanOrEqual(box.width)
    for (const lane of band.lanes) {
      expect(lane.boxes, label).toBeGreaterThan(0)
      const room = lane.columns * lane.lines * lane.pile
      expect(room, `${label}: room for every box`).toBeGreaterThanOrEqual(lane.boxes)
      expect(lane.span).toBe(lane.columns * (lane.width + gap) - gap)
      expect(lane.width).toBe(Math.round(fit.cardWidth * (lane.lane === 'creatures' ? 1 : LAND_SCALE)))
      expect(lane.height).toBe(Math.round(lane.width * CARD_ASPECT))
      const tall =
        lane.pile > 1
          ? lane.height + (lane.pile - 1) * lane.head
          : lane.lines * lane.height + (lane.lines - 1) * rowGap
      expect(lane.tall, label).toBe(tall)
      expect(tall).toBeLessThanOrEqual(band.height)
      // Only lands fan, and a covered land shows at least its name line.
      if (lane.pile > 1) expect([lane.lane, lane.lines]).toEqual(['lands', 1])
      if (lane.pile > 1) expect(lane.head).toBeGreaterThanOrEqual(LANE_LIMITS.fanHeadMin)
    }
  }
  const height =
    fit.bands.reduce((sum, band) => sum + band.height, 0) + bandGap * Math.max(0, fit.bands.length - 1)
  expect(fit.height).toBe(height)
  if (!fit.scrolls) expect(height, `${label}: height`).toBeLessThanOrEqual(box.height)
}

const BOXES: readonly FitBox[] = [
  { width: 1100, height: 330 },
  { width: 900, height: 290 },
  { width: 350, height: 200 },
  { width: 360, height: 300 },
  { width: 1400, height: 380 },
]
const COUNTS: readonly LaneCounts[] = [
  counts(0, 0, 1),
  counts(1, 0, 0),
  counts(0, 3, 0),
  counts(2, 1, 3),
  counts(4, 3, 5),
  counts(5, 3, 4),
  counts(14, 10, 6),
  counts(30, 20, 16),
]

describe('fitLanes', () => {
  it.each(BOXES.flatMap((box) => COUNTS.map((lanes) => [box, lanes] as const)))(
    'fits %o with %o and nothing larger fits',
    (box, lanes) => {
      const label = `${box.width}x${box.height} ${JSON.stringify(lanes)}`
      const fit = fitLanes(box, lanes, 'near')
      expect(ladder()).toContain(fit.cardWidth)
      holds(fit, box, label)
      const placed = Object.fromEntries(lanesOf(fit).map((lane) => [lane.lane, lane.boxes]))
      for (const lane of ['creatures', 'other', 'lands'] as const) {
        // An empty lane takes no room: it has no place in any band.
        expect(placed[lane] ?? 0, label).toBe(lanes[lane])
      }
      if (fit.scrolls || fit.cardWidth === LANE_LIMITS.maxWidth) return
      // The next rung of the ladder fits no shape.
      const higher = { ...LANE_LIMITS, minWidth: fit.cardWidth + LANE_LIMITS.step }
      expect(fitLanes(box, lanes, 'near', higher).scrolls, label).toBe(true)
    },
  )

  it.each(COUNTS)('never gives smaller cards to a bigger box or to fewer permanents: %o', (lanes) => {
    for (const box of BOXES) {
      const fit = fitLanes(box, lanes, 'near')
      const wider = fitLanes({ width: box.width + 120, height: box.height }, lanes, 'near')
      const taller = fitLanes({ width: box.width, height: box.height + 90 }, lanes, 'near')
      expect(wider.cardWidth).toBeGreaterThanOrEqual(fit.cardWidth)
      expect(taller.cardWidth).toBeGreaterThanOrEqual(fit.cardWidth)
      const fewer = { ...lanes, creatures: Math.max(0, lanes.creatures - 1) }
      expect(fitLanes(box, fewer, 'near').cardWidth).toBeGreaterThanOrEqual(fit.cardWidth)
    }
  })

  it.each([
    ['one card in a large board', { width: 1100, height: 400 }, counts(1, 0, 0), 224, 'two'],
    ['a few permanents side by side', { width: 1100, height: 330 }, counts(2, 1, 3), 224, 'one'],
    ['an early board in two bands', { width: 500, height: 330 }, counts(3, 2, 3), 118, 'two'],
  ])('%s', (_name, box, lanes, width, shape) => {
    const fit = fitLanes(box, lanes, 'near')
    expect(fit).toMatchObject({ cardWidth: width, shape, scrolls: false, hidden: 0 })
  })

  it('mirrors an opponent: creatures face the middle of the table on both sides', () => {
    const box = { width: 500, height: 330 }
    const lanes = counts(3, 2, 3)
    const near = fitLanes(box, lanes, 'near')
    const far = fitLanes(box, lanes, 'far')
    expect(far.cardWidth).toBe(near.cardWidth)
    expect(near.bands.map((band) => band.lanes.map((lane) => lane.lane))).toEqual([
      ['creatures'],
      ['lands', 'other'],
    ])
    expect(far.bands.map((band) => band.lanes.map((lane) => lane.lane))).toEqual([
      ['lands', 'other'],
      ['creatures'],
    ])
  })

  it('scrolls at the floor with the creatures first and counts what is cut off', () => {
    const fit = fitLanes({ width: 300, height: 90 }, counts(14, 10, 16), 'far')
    expect(fit).toMatchObject({ cardWidth: LANE_LIMITS.minWidth, scrolls: true })
    expect(fit.hidden).toBeGreaterThan(0)
    expect(fit.bands[0]?.lanes.map((lane) => lane.lane)).toContain('creatures')
    holds(fit, { width: 300, height: 90 }, 'floor')
  })

  // `fanHead: 1` shows a whole land above the next, which never beats plain lines: the fit without fanning.
  const PLAIN: LaneLimits = { ...LANE_LIMITS, fanHead: 1 }

  it.each([
    [{ width: 930, height: 200 }, counts(4, 2, 4)],
    [{ width: 700, height: 260 }, counts(3, 2, 9)],
  ])('fans the lands into piles in %o when that buys a larger card: %o', (box, lanes) => {
    const fit = fitLanes(box, lanes, 'near')
    const plain = fitLanes(box, lanes, 'near', PLAIN)
    expect(fit.cardWidth).toBeGreaterThan(plain.cardWidth)
    const land = lanesOf(fit).find((lane) => lane.lane === 'lands')
    expect(land?.pile).toBeGreaterThan(1)
    holds(fit, box, 'fanned')
    expect(lanesOf(plain).every((lane) => lane.pile === 1)).toBe(true)
  })

  it.each(COUNTS)('keeps plain lanes where fanning buys nothing: %o', (lanes) => {
    for (const box of BOXES) {
      const fit = fitLanes(box, lanes, 'near')
      const plain = fitLanes(box, lanes, 'near', PLAIN)
      expect(fit.cardWidth).toBeGreaterThanOrEqual(plain.cardWidth)
      if (fit.cardWidth === plain.cardWidth && fit.shape === plain.shape)
        expect(fit.height).toBeLessThanOrEqual(plain.height)
    }
  })

  it('answers for an empty board and a box of no size', () => {
    expect(fitLanes({ width: 800, height: 300 }, counts(0, 0, 0), 'near')).toMatchObject({
      cardWidth: LANE_LIMITS.maxWidth,
      bands: [],
      height: 0,
      scrolls: false,
    })
    const none = fitLanes({ width: 0, height: 0 }, counts(2, 1, 1), 'near')
    expect(none).toMatchObject({ cardWidth: LANE_LIMITS.minWidth, scrolls: true })
    expect(lanesOf(none).every((lane) => lane.columns === 1)).toBe(true)
  })
})
