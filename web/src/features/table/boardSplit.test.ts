//! Goal: the board rows split by what each board holds, so the duel board draws large cards and no board scrolls.
import { describe, expect, it } from 'vitest'
import { fixtureState } from './dev/devGame'
import {
  boardCounts,
  boardFrames,
  MIN_ROW_SHARE,
  shareSplit,
  splitBoards,
  splitRegions,
  THEIR_SCALE,
} from './boardSplit'
import { buildFixture } from './dev/fixture'
import { laneHeight } from './laneFit'
import type { LaneCounts } from './laneFit'
import { planTable } from './layout'
import { battlefieldStacks, layoutSeat } from './rowStacks'

const counts = (lands: number, creatures: number, other: number): LaneCounts => ({ lands, creatures, other })
const NONE = counts(0, 0, 0)

describe('splitBoards', () => {
  const base = { room: 600, ownChrome: 24, theirChrome: 24, ownWidth: 900, theirWidth: 900 }

  it.each([
    ['both empty', NONE, [NONE]],
    ['nothing fits at the floor', counts(40, 40, 40), [counts(40, 40, 40)]],
  ])('splits by the default shares when %s', (_, own, theirs) => {
    const split = splitBoards({ ...base, room: 100, own, theirs })
    expect(split).toEqual(shareSplit(100))
  })

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
