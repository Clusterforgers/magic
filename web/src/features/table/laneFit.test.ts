// Goal: every permanent lands in its lane by type, and the lane fit picks the largest card of the ladder at which every lane of a board fits its box without overlap, keeps the card shape, takes no room for an empty lane, mirrors an opponent's bands and grows with the room.
import { describe, expect, it } from 'vitest'
import type { CardType, ObjectView, Subtype } from '@/protocol/generated'
import { card } from '@/test-utils/cards'
import { rowOf } from './cardModel'
import type { RowKind } from './cardModel'
import { fixtureState } from './dev/devGame'
import { buildFixture } from './dev/fixture'
import { CARD_ASPECT, LAND_SCALE } from './fit'
import type { FitBox } from './fit'
import { fitLanes, LANE_LIMITS } from './laneFit'
import type { LaneCounts, LaneFit, LaneLimits } from './laneFit'
import { POD_COLUMN_PX, regionSizes, ZONE_GAP_PX } from './layout'
import { battlefieldStacks, laneCounts, layoutSeat } from './rowStacks'

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
      expect(lane.columns * lane.lines, `${label}: room for every box`).toBeGreaterThanOrEqual(lane.boxes)
      expect(lane.span).toBe(lane.columns * (lane.width + gap) - gap)
      expect(lane.width).toBe(Math.round(fit.cardWidth * (lane.lane === 'lands' ? LAND_SCALE : 1)))
      expect(lane.height).toBe(Math.round(lane.width * CARD_ASPECT))
      expect(lane.lines * lane.height + (lane.lines - 1) * rowGap).toBeLessThanOrEqual(band.height)
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
    ['a few permanents side by side', { width: 1100, height: 330 }, counts(2, 1, 3), 194, 'one'],
    ['an early board in two bands', { width: 500, height: 330 }, counts(3, 2, 3), 104, 'two'],
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

// A board beside a 168 px pod column inside a region padded by 8 px, in a zone padded by 4 px (`SeatArea`, `Battlefield`).
function zones(width: number, height: number): { own: FitBox; opponent: FitBox } {
  const regions = regionSizes({ width, height })
  const inner = regions.main.width - 2 * ZONE_GAP_PX - POD_COLUMN_PX - ZONE_GAP_PX - 8
  const zone = (region: number): FitBox => ({
    width: Math.floor(inner),
    height: Math.floor(region - 2 * ZONE_GAP_PX - 8),
  })
  return { own: zone(regions.own), opponent: zone(regions.opponents) }
}

describe('the duel board of an early Commander game', () => {
  const state = fixtureState(buildFixture({ seats: 2, board: 'duel' }))
  const fitOf = (seat: number, box: FitBox, facing: 'near' | 'far') => {
    const stacks = battlefieldStacks(state, seat)
    const fit = layoutSeat({ box, stacks, facing, opened: new Set(), marked: new Set() }).fit
    if (fit === null) throw new Error('a box was given')
    return fit
  }

  it('holds 12 permanents of yours and 10 of the opponent', () => {
    const permanents = (seat: number) =>
      Object.values(
        laneCounts(
          layoutSeat({
            box: null,
            stacks: battlefieldStacks(state, seat, false),
            facing: 'near',
            opened: new Set(),
            marked: new Set(),
          }).rows,
        ),
      ).reduce((sum, count) => sum + count, 0)
    expect([permanents(0), permanents(1)]).toEqual([12, 10])
  })

  it.each([
    [1920, 1080, 130, 110],
    [1440, 900, 100, 80],
    [1366, 768, 84, 66],
    [1024, 768, 84, 66],
  ])('draws cards readable without a hover at %ix%i', (width, height, ownMin, theirMin) => {
    const { own, opponent } = zones(width, height)
    const mine = fitOf(0, own, 'near')
    const theirs = fitOf(1, opponent, 'far')
    expect(mine.scrolls || theirs.scrolls).toBe(false)
    expect(mine.cardWidth).toBeGreaterThanOrEqual(ownMin)
    expect(theirs.cardWidth).toBeGreaterThanOrEqual(theirMin)
  })
})
