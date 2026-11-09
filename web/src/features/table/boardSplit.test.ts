//! Goal: the board rows split by what each board holds, so the duel board draws large cards and no board scrolls, and the opponents keep their pods and a row of cards under a prompt sheet.
import { describe, expect, it } from 'vitest'
import { fixtureState } from './dev/devGame'
import {
  BOARD_KEEP_PX,
  boardCounts,
  boardFrames,
  MIN_ROW_SHARE,
  shareSplit,
  splitBoards,
  splitRegions,
  THEIR_SCALE,
  theirChromeOf,
  withSplit,
} from './boardSplit'
import { buildFixture } from './dev/fixture'
import { laneHeight } from './laneFit'
import type { LaneCounts } from './laneFit'
import { planTable, regionSizes, SHEET_KEEP_PX } from './layout'
import { battlefieldStacks, layoutSeat } from './rowStacks'
import { headOf } from './useTheirHead'

const counts = (lands: number, creatures: number, other: number): LaneCounts => ({ lands, creatures, other })
const NONE = counts(0, 0, 0)

describe('splitBoards', () => {
  const base = { room: 600, ownChrome: 24, theirChrome: 24, ownWidth: 900, theirWidth: 900 }

  it.each([
    ['both empty', NONE, [NONE]],
    ['nothing fits at the floor', counts(40, 40, 40), [counts(40, 40, 40)]],
  ])('splits by the default shares when %s', (_, own, theirs) => {
    const split = splitBoards({ ...base, room: 400, own, theirs })
    expect(split).toEqual(shareSplit(400))
  })

  it.each([
    // The opponents' row keeps its chrome and a row of cards, and your row a quarter of the room.
    [600, 260, 324],
    [400, 200, 264],
    [400, 300, 300],
  ])(
    'lifts the opponents of a %i px room with %i px of chrome to %f px when nothing fits',
    (room, chrome, most) => {
      const crowded = counts(40, 40, 40)
      const split = splitBoards({ ...base, room, theirChrome: chrome, own: crowded, theirs: [crowded] })
      expect(split.opponents).toBeCloseTo(most, 6)
      expect(split.opponents + split.own).toBeCloseTo(room, 6)
      expect(split.own).toBeGreaterThanOrEqual(room * MIN_ROW_SHARE)
    },
  )

  it.each([
    [counts(4, 5, 3), [counts(4, 4, 2)]],
    [counts(8, 12, 6), [counts(3, 1, 0)]],
    [counts(2, 1, 0), [counts(10, 14, 8)]],
    [counts(6, 6, 6), [counts(4, 4, 2), counts(7, 2, 1), counts(0, 0, 0)]],
  ])('fits both boards at the largest card, the opponents at four fifths: %o against %o', (own, theirs) => {
    const split = splitBoards({ ...base, own, theirs })
    expect(split.own + split.opponents).toBeCloseTo(base.room)
    expect(split.theirCard).toBeGreaterThanOrEqual(Math.floor(split.ownCard * THEIR_SCALE))
    expect(split.theirCard).toBeLessThanOrEqual(split.ownCard)
    const theirNeed = Math.max(...theirs.map((each) => laneHeight(base.theirWidth, each, split.theirCard)))
    expect(laneHeight(base.ownWidth, own, split.ownCard)).toBeLessThanOrEqual(split.own - base.ownChrome)
    expect(theirNeed).toBeLessThanOrEqual(split.opponents - base.theirChrome)
    expect(Math.min(split.own, split.opponents)).toBeGreaterThanOrEqual(base.room * MIN_ROW_SHARE - 0.001)
    // One rung more does not fit both rows, each at least its share.
    const more = split.ownCard + 2
    const least = base.room * MIN_ROW_SHARE
    const theirMore = Math.max(
      ...theirs.map((each) => laneHeight(base.theirWidth, each, Math.floor(more * THEIR_SCALE))),
    )
    const ownRow = Math.max(least, laneHeight(base.ownWidth, own, more) + base.ownChrome)
    expect(ownRow + Math.max(least, theirMore + base.theirChrome)).toBeGreaterThan(base.room)
  })

  it('gives a crowded board the room a sparse one lends', () => {
    const crowdedOwn = splitBoards({ ...base, own: counts(8, 12, 6), theirs: [counts(3, 1, 0)] })
    const crowdedTheirs = splitBoards({ ...base, own: counts(3, 1, 0), theirs: [counts(8, 12, 6)] })
    expect(crowdedOwn.own).toBeGreaterThan(crowdedTheirs.own)
  })
})

const VIEWPORTS = [
  [1920, 1080, 190, 160, 0],
  [1920, 1080, 190, 160, 40],
  [1440, 900, 150, 120, 0],
  // The two lines of the priority prompt at 1440 stand 50 px above the strip.
  [1440, 900, 150, 120, 56],
  [1366, 768, 110, 88, 0],
  [1280, 720, 100, 80, 0],
  [1024, 768, 96, 76, 0],
] as const

/** The creature card width of each board of a fixture at a window size, fitted as the table does. */
function cardsAt(width: number, height: number, seats: number, board: 'duel' | 'heavy', reserve = 0) {
  const state = fixtureState(buildFixture({ seats, board, permanents: board === 'heavy' ? 20 : undefined }))
  const table = planTable({ viewport: { width, height }, seatCount: seats, viewer: 0 })
  const frames = boardFrames(table.regions, table.seats)
  const stacksOf = (seat: number) => battlefieldStacks(state, seat)
  const regions = splitRegions(table.regions, table.seats, boardCounts(state, table.seats), reserve)
  const theirFrame = { ...frames.theirs, chrome: frames.theirs.chrome + reserve }
  const fit = (
    seat: number,
    row: number,
    frame: { width: number; chrome: number },
    facing: 'near' | 'far',
  ) => {
    const box = { width: Math.floor(frame.width), height: Math.floor(row - frame.chrome) }
    const layout = layoutSeat({ box, stacks: stacksOf(seat), facing, opened: new Set(), marked: new Set() })
    if (layout.fit === null) throw new Error('a box was given')
    return layout.fit
  }
  const own = fit(0, regions.own, frames.own, 'near')
  const theirs = table.seats.boards.map((seat) => fit(seat, regions.opponents, theirFrame, 'far'))
  return { own, theirs }
}

describe('the duel board of an early Commander game', () => {
  it.each(VIEWPORTS)(
    'draws your creatures and the opponent large at %ix%i: yours %i px, theirs %i, with %i px kept for a prompt',
    (width, height, ownMin, theirMin, reserve) => {
      const { own, theirs } = cardsAt(width, height, 2, 'duel', reserve)
      expect([own.scrolls, ...theirs.map((fit) => fit.scrolls)]).toEqual([false, false])
      expect(own.cardWidth).toBeGreaterThanOrEqual(ownMin)
      expect(Math.min(...theirs.map((fit) => fit.cardWidth))).toBeGreaterThanOrEqual(theirMin)
    },
  )
})

describe('the prompt sheet of a split table', () => {
  it.each([
    [360, 740, true],
    [390, 844, true],
    [1400, 900, false],
  ])(
    'on %ix%i leaves a phone the tile strip and a row of the opponents (phone: %s)',
    (width, height, phone) => {
      const regions = withSplit(regionSizes({ width, height }), {
        opponents: 250,
        own: 300,
        ownCard: 100,
        theirCard: 80,
      })
      expect(regions.phone).toBe(phone)
      const room = phone ? 250 - SHEET_KEEP_PX : 250
      expect(regions.sheet).toBeCloseTo(Math.min(height * 0.4, regions.shared + room), 6)
    },
  )

  it.each([
    [1280, 720, 192],
    [1366, 768, 160],
    [1024, 768, 72],
  ])(
    'on %ix%i leaves the opponents their pods and a row of cards (%i px of chrome)',
    (width, height, chrome) => {
      const regions = withSplit(
        regionSizes({ width, height }),
        { opponents: 320, own: 200, ownCard: 60, theirCard: 48 },
        chrome + BOARD_KEEP_PX,
      )
      expect(regions.sheet).toBeCloseTo(
        Math.min(height * 0.4, regions.shared + 320 - chrome - BOARD_KEEP_PX),
        6,
      )
    },
  )
})

describe('the chrome of the opponents row', () => {
  it.each([
    // No board measured, or pods no taller than the estimate: the estimate.
    [72, 0, 72],
    [72, 40, 72],
    // A pod line wrapped to four lines: the measured head with the zone's padding and the row's gap, in steps of 8.
    [72, 174, 192],
    [72, 177, 200],
    [72, Number.NaN, 72],
  ])('of an estimate of %i px with battlefields %f px down is %i px', (estimate, head, chrome) => {
    expect(theirChromeOf(estimate, head)).toBe(chrome)
  })

  it('makes the opponents row of a short window keep their pods and a row of cards under a long prompt', () => {
    const viewport = { width: 1280, height: 720 }
    const table = planTable({ viewport, seatCount: 4, viewer: 0 })
    const state = fixtureState(buildFixture({ seats: 4 }))
    const regions = splitRegions(table.regions, table.seats, boardCounts(state, table.seats), 64, 174)
    expect(regions.opponents).toBeGreaterThanOrEqual(192 + BOARD_KEEP_PX)
    expect(regions.opponents - (regions.sheet - regions.shared)).toBeGreaterThanOrEqual(
      192 + BOARD_KEEP_PX - 1e-9,
    )
    expect(regions.sheet).toBeGreaterThan(regions.shared + 32)
  })
})

describe('the head of the opponents area', () => {
  it.each([
    [44, [], 0],
    [44, [218, 190, 246], 202],
    [100, [80], 0],
    [44, [Number.NaN, 120], 76],
  ])('of an area at %i px over zones at %o is %i px', (top, zones, head) => {
    expect(headOf(top, zones)).toBe(head)
  })
})

describe('a four-seat table', () => {
  it.each([
    [1920, 1080, 'duel', 120, 90],
    [1440, 900, 'duel', 100, 70],
    [1920, 1080, 'heavy', 64, 44],
  ] as const)(
    'keeps every board readable at %ix%i on the %s board',
    (width, height, board, ownMin, theirMin) => {
      const { own, theirs } = cardsAt(width, height, 4, board)
      expect(own.scrolls).toBe(false)
      expect(own.cardWidth).toBeGreaterThanOrEqual(ownMin)
      expect(Math.min(...theirs.map((fit) => fit.cardWidth))).toBeGreaterThanOrEqual(theirMin)
    },
  )
})
