//! Goal: the board split of the table is a fixed point of what it measures, so the layout settles in one pass at every seat count from 2 to 8, in grid and default mode, at 1280x720, 1920x1080 and 2180x1290, and the grid's boards share one card size.
import { describe, expect, it } from 'vitest'
import { fixtureState } from './dev/devGame'
import { buildFixture } from './dev/fixture'
import { boardCounts, boardFrames, splitRegions } from './boardSplit'
import { gridMost } from './gridCard'
import { planTable } from './layout'
import type { TableMode } from './layout'
import { battlefieldStacks, layoutSeat } from './rowStacks'
import { headOf } from './useTheirHead'

/** The opponents' area's top padding, a frame's border and padding, and the gap under a pod line (`Seats.tsx`). */
const AREA_PAD = 8
const FRAME_EDGE = 6
const GAP = 8

/**
 * Where the opponents' battlefields begin, as the page would measure it after drawing `opponents` px of area for
 * `rows` rows of frames whose pod lines are `pods` px high (a seat's pod wraps to more lines than another's).
 */
function measuredHead(opponents: number, rows: number, pods: readonly number[], columns: number): number {
  const rowHeight = (opponents - 2 * AREA_PAD - (rows - 1) * GAP) / rows
  const zones = pods.map((pod, index) => {
    const frame = AREA_PAD + Math.floor(index / columns) * (rowHeight + GAP)
    return { top: frame + FRAME_EDGE + pod + GAP, frame }
  })
  return headOf(0, zones)
}

const SIZES = [
  [1280, 720],
  [1920, 1080],
  [2180, 1290],
] as const
const SEATS = [2, 3, 4, 5, 6, 7, 8] as const
const MODES: readonly (TableMode | undefined)[] = ['grid', undefined]

const cases = SEATS.flatMap((seats) =>
  SIZES.flatMap(([width, height]) => MODES.map((mode) => [seats, width, height, mode ?? 'default'] as const)),
)

describe('the board split', () => {
  it.each(cases)('settles at %i seats in %ix%i (%s mode)', (seats, width, height, mode) => {
    const state = fixtureState(buildFixture({ seats }))
    const table = planTable({
      viewport: { width, height },
      seatCount: seats,
      viewer: 0,
      ...(mode === 'default' ? {} : { mode }),
    })
    const plan = table.seats
    const counts = boardCounts(state, plan)
    const rows = boardFrames(table.regions, plan).theirs.rows ?? 1
    const pods = plan.boards.map((_, index) => (index % 2 === 0 ? 88 : 64))
    // Start as the page does, with no head measured, and apply the split to what it draws, twice.
    let head = 0
    const passes = [0, 1, 2, 3].map(() => {
      const regions = splitRegions(table.regions, plan, counts, 0, head)
      head = measuredHead(regions.opponents, rows, pods, plan.columns)
      return { opponents: regions.opponents, own: regions.own, head }
    })
    // One pass to learn the head, then the split gives its own output back.
    expect(passes[2]).toEqual(passes[1])
    expect(passes[3]).toEqual(passes[1])
  })
})

describe('the head of a grid of frames', () => {
  it.each([400, 600, 900])('does not follow the rows of a %i px area', (opponents) => {
    expect(measuredHead(opponents, 2, [88, 64, 64, 64, 64, 64], 3)).toBe(FRAME_EDGE + 88 + GAP + AREA_PAD)
  })
})

describe('the grid card size', () => {
  it.each([
    [[1, 2, 3], new Map([[1, 90]]), 90],
    [[1, 2, 3], new Map(), null],
    [
      [1, 2, 3],
      new Map([
        [1, 90],
        [2, 60],
        [3, 120],
      ]),
      60,
    ],
    // A seat no longer in the grid does not count.
    [
      [1, 3],
      new Map([
        [1, 90],
        [2, 44],
      ]),
      90,
    ],
  ])('of seats %o reporting %o is %o', (seats, widths, most) => {
    expect(gridMost(seats, widths)).toBe(most)
  })

  it.each(SIZES)('is a fixed point for the boards of a 7-seat grid at %ix%i', (width, height) => {
    const state = fixtureState(buildFixture({ seats: 7 }))
    const table = planTable({ viewport: { width, height }, seatCount: 7, viewer: 0, mode: 'grid' })
    const regions = splitRegions(table.regions, table.seats, boardCounts(state, table.seats))
    const frame = boardFrames(table.regions, table.seats).theirs
    const rows = frame.rows ?? 1
    const box = {
      width: Math.floor(frame.width),
      height: Math.floor(regions.opponents / rows - frame.chrome),
    }
    const fitOf = (seat: number, most: number | null) =>
      layoutSeat({
        box,
        stacks: battlefieldStacks(state, seat),
        facing: 'far',
        opened: new Set(),
        marked: new Set(),
        most,
      }).fit?.cardWidth ?? null
    const alone = new Map(table.seats.boards.map((seat) => [seat, fitOf(seat, null) ?? 0]))
    const most = gridMost(table.seats.boards, alone)
    const shared = table.seats.boards.map((seat) => fitOf(seat, most))
    // Every board draws the one size, and a board's own fit (what it reports) does not move under the cap.
    expect(new Set(shared).size).toBe(1)
    expect(shared[0]).toBe(most)
    expect(
      gridMost(table.seats.boards, new Map(table.seats.boards.map((seat) => [seat, fitOf(seat, null) ?? 0]))),
    ).toBe(most)
  })
})
