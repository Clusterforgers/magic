// Goal: the boss-mode split shares the table's height by what each board draws: the rows add up to the room, a board with more permanents gets more, the boss's side stays under its cap and its cards under the heroes', a board without permanents is one line, a hand that will fill an empty board keeps your row, and a room too small for boards leaves the allies a strip of tiles; the hand dock of a seat with nothing to play is a slim bar in boss mode only.
import { describe, expect, it } from 'vitest'
import {
  ALLY_LINE_PX,
  ALLY_MIN_SHARE,
  ALLY_ROW_MIN_PX,
  BOSS_LINE_PX,
  BOSS_SCALE,
  BOSS_SIDE_MOST,
  OWN_LINE_PX,
  allyShares,
  permanentsIn,
  splitBoss,
} from './bossSplit'
import type { BossBoard, BossSplitInput } from './bossSplit'
import { LANE_LIMITS, ladderOf, laneHeight } from './laneFit'
import type { LaneCounts } from './laneFit'
import { must } from '@/test-utils/batches'
import { bossSplitOf } from './bossRows'
import { fixtureState, withBoss } from './dev/devGame'
import { buildFixture } from './dev/fixture'
import { seatCounts } from './boardSplit'
import { bossSidesOf } from './selectors'
import {
  ALLY_STRIP_PX,
  HAND_SLIM_PX,
  POD_COLUMN_PX,
  ZONE_GAP_PX,
  gridRows,
  planTable,
  regionSizes,
} from './layout'
import type { BossSides } from './layout'

const board = (width: number, counts: LaneCounts, head?: number): BossBoard =>
  head === undefined ? { width, counts } : { width, counts, head }

/** About `n` permanents: 40 % lands, 35 % creatures, the rest others, as a mid-game board is. */
function permanents(n: number): LaneCounts {
  const lands = Math.round(n * 0.4)
  const creatures = Math.round(n * 0.35)
  return { lands, creatures, other: Math.max(0, n - lands - creatures) }
}

const NONE: LaneCounts = { lands: 0, creatures: 0, other: 0 }

interface Shape {
  room?: number
  boss?: number
  allies?: number[]
  own?: number
  ready?: boolean
  minions?: number[]
}

/** The input of a window about 1130 px wide: the boss's board 560 wide, allies and your board by the share of the row. */
function input({
  room = 650,
  boss = 15,
  allies = [10, 10],
  own = 10,
  ready = true,
  minions = [],
}: Shape = {}): BossSplitInput {
  const shares = allyShares(allies.map(permanents))
  const sum = shares.reduce((a, b) => a + b, 0)
  return {
    room,
    boss: [board(560, permanents(boss)), ...minions.map((count) => board(260, permanents(count), 48))],
    bossChrome: 76,
    bossFloor: 196,
    allies: allies.map((count, at) =>
      board(((1112 - 8 * allies.length) * (shares[at] ?? 1)) / sum - 20, permanents(count)),
    ),
    allyChrome: 80,
    own: board(930, permanents(own)),
    ownChrome: 24,
    ownReady: ready,
    ownFloor: 136,
    gap: 8,
  }
}

describe('splitBoss', () => {
  it.each([480, 560, 650, 800, 1100, 1500])('the rows add up to a room of %i px', (room) => {
    for (const allies of [[], [10], [10, 3], [4, 4, 12]]) {
      const split = splitBoss(input({ room, allies }))
      expect(split.opponents + split.own).toBeCloseTo(room, 5)
      expect(split.band).toBeLessThanOrEqual(split.own)
      expect(split.opponents).toBeLessThanOrEqual(room * BOSS_SIDE_MOST + 1e-6)
      if (allies.length === 0) expect(split.band).toBe(0)
    }
  })

  it.each([
    // [boss permanents, minions, room]
    [15, [], 650],
    [40, [], 650],
    [60, [12, 12], 900],
    [20, [], 1100],
  ] as const)('caps the boss side: %i permanents, minions %j, room %i', (boss, minions, room) => {
    const split = splitBoss(input({ room, boss, minions: [...minions] }))
    expect(split.opponents).toBeLessThanOrEqual(room * BOSS_SIDE_MOST + 1e-6)
    // Its cards are never larger than the heroes'.
    expect(split.bossCard).toBeLessThanOrEqual(Math.floor(split.heroCard * BOSS_SCALE))
    expect(split.bossCard).toBeLessThanOrEqual(split.heroCard)
  })

  it('gives a board with more permanents more room', () => {
    const few = splitBoss(input({ allies: [4, 4] }))
    const many = splitBoss(input({ allies: [14, 14] }))
    expect(many.band).toBeGreaterThan(few.band)
    const lean = splitBoss(input({ own: 4, allies: [14, 14] }))
    const crowded = splitBoss(input({ own: 24, allies: [14, 14] }))
    expect(crowded.own - crowded.band).toBeGreaterThan(lean.own - lean.band)
  })

  it.each([
    [[10, 10], 690],
    [[10], 650],
  ])(
    "draws the allies %j at 100 px cards or more in a room of %i px beside a Spirit's line",
    (allies, room) => {
      const given = input({ room, allies, own: 0, ready: false })
      const split = splitBoss(given)
      const most = Math.max(...given.allies.map((ally) => laneHeight(ally.width, ally.counts, 100)))
      expect(split.band).toBeGreaterThanOrEqual(given.allyChrome + most)
      expect(split.band).toBeGreaterThanOrEqual(ALLY_ROW_MIN_PX)
    },
  )

  it('takes one line for a board without permanents and gives the rest to the boards that draw some', () => {
    const bare = splitBoss(input({ boss: 0, allies: [0, 0], own: 0, ready: false }))
    expect(bare.opponents).toBe(BOSS_LINE_PX)
    expect(bare.band).toBe(ALLY_LINE_PX)
    expect(bare.own - bare.band - 8).toBeGreaterThanOrEqual(OWN_LINE_PX)
    // A Spirit's empty row is a line; the ally that plays takes the room instead.
    const spirit = splitBoss(input({ own: 0, ready: false }))
    expect(spirit.own - spirit.band - 8).toBe(OWN_LINE_PX)
    const alone = splitBoss(input({ own: 0, ready: false, allies: [12] }))
    expect(alone.band).toBeGreaterThan(splitBoss(input({ own: 12, allies: [12] })).band)
  })

  it('keeps room for your board while your hand can still fill it', () => {
    const waiting = splitBoss(input({ own: 0, ready: true }))
    const idle = splitBoss(input({ own: 0, ready: false }))
    expect(waiting.own - waiting.band - 8).toBeGreaterThan(OWN_LINE_PX)
    expect(waiting.band).toBeLessThan(idle.band)
  })

  it.each([200, 260, 340])(
    'squeezes into a room of %i px: boss capped, allies a strip, nothing over the room',
    (room) => {
      const split = splitBoss(input({ room, boss: 40, allies: [20, 20], own: 30 }))
      expect(split.opponents + split.own).toBeCloseTo(room, 5)
      expect(split.opponents).toBeLessThanOrEqual(room * BOSS_SIDE_MOST + 1e-6)
      expect(split.band === ALLY_STRIP_PX || split.band >= ALLY_ROW_MIN_PX).toBe(true)
      expect(split.heroCard).toBe(0)
    },
  )

  it.each([2, 5])(
    'squeezed with %i crowded allies, your board keeps what it needs at the floor before the allies grow',
    (count) => {
      for (const room of [480, 600, 700]) {
        const split = splitBoss(
          input({ room, boss: 40, allies: Array.from({ length: count }, () => 40), own: 40 }),
        )
        const yours = split.own - split.band - 8
        const need = 24 + laneHeight(930, permanents(40), ladderOf(LANE_LIMITS).at(0))
        expect(yours).toBeGreaterThanOrEqual(Math.min(need, split.own - 8 - ALLY_ROW_MIN_PX) - 1e-6)
      }
    },
  )

  // The dev route's fight (`/dev/table?seats=4&boss=0`): three heroes of 40 permanents, squeezed or not.
  it.each([
    [2, 1024, 768],
    [3, 1024, 768],
    [3, 1280, 720],
    [3, 1400, 900],
    [3, 1530, 898],
    [3, 1873, 1047],
    [4, 1400, 900],
  ])(
    '%i heroes at 40 permanents at %i x %i: your lanes get the height they need',
    (heroes, width, height) => {
      const state = fixtureState(withBoss(buildFixture({ seats: heroes + 1 }), 0))
      const plan = planTable({
        viewport: { width, height },
        seatCount: heroes + 1,
        viewer: 0,
        sides: must(bossSidesOf(must(state.view))),
      })
      const regions = plan.regions
      const counts = {
        own: seatCounts(state, 0),
        theirs: plan.seats.boards.map((seat) => seatCounts(state, seat)),
        allies: plan.seats.allies.map((seat) => seatCounts(state, seat)),
        ownReady: true,
      }
      const { split } = bossSplitOf(regions, plan.seats, counts, 0, 0)
      const gap = counts.allies.length === 0 ? 0 : ZONE_GAP_PX
      // Your row less the allies' row, its gap, the row's padding and the board's.
      const lanes = split.own - split.band - gap - 2 * ZONE_GAP_PX - 2 * 4
      const ownWidth = regions.main.width - 2 * ZONE_GAP_PX - POD_COLUMN_PX - ZONE_GAP_PX - 2 * 4
      const card = split.heroCard > 0 ? split.heroCard : ladderOf(LANE_LIMITS).at(0)
      expect(lanes).toBeGreaterThanOrEqual(laneHeight(ownWidth, counts.own, card) - 1e-6)
    },
  )

  it('is total for a room that is not a number', () => {
    for (const room of [Number.NaN, -5, 0, Number.POSITIVE_INFINITY]) {
      const split = splitBoss(input({ room }))
      expect(Number.isFinite(split.opponents)).toBe(true)
      expect(Number.isFinite(split.own)).toBe(true)
    }
  })
})

describe('allyShares', () => {
  it('weighs a column by its permanents, at least ALLY_MIN_SHARE', () => {
    expect(allyShares([NONE, permanents(2), permanents(10), permanents(30)])).toEqual([
      ALLY_MIN_SHARE,
      ALLY_MIN_SHARE,
      permanentsIn(permanents(10)),
      permanentsIn(permanents(30)),
    ])
  })
})

describe('the slim hand dock', () => {
  const sides: BossSides = { boss: 2, enemies: [2] }
  const windows = [
    { width: 2550, height: 1430 },
    { width: 1400, height: 900 },
    { width: 1280, height: 720 },
  ]

  it.each(windows)('is HAND_SLIM_PX high in boss mode with nothing in the hand at %j', (viewport) => {
    const plain = planTable({ viewport, seatCount: 3, viewer: 0, sides })
    const slim = planTable({ viewport, seatCount: 3, viewer: 0, sides, handSlim: true })
    expect(slim.regions.handSlim).toBe(true)
    expect(slim.regions.hand).toBe(HAND_SLIM_PX)
    expect(slim.regions.hand).toBeLessThanOrEqual(plain.regions.hand)
    // The boards get what the dock gives up.
    expect(slim.regions.opponents + slim.regions.own).toBeCloseTo(
      plain.regions.opponents + plain.regions.own + plain.regions.hand - HAND_SLIM_PX,
      5,
    )
    expect(gridRows(slim.regions)).toMatch(new RegExp(` ${HAND_SLIM_PX}px$`))
    expect(gridRows(plain.regions)).not.toMatch(new RegExp(` ${HAND_SLIM_PX}px$`))
  })

  it('leaves every other table as it is: grid mode, focus mode and a phone', () => {
    const viewport = { width: 1400, height: 900 }
    for (const input of [
      { viewport, seatCount: 3, viewer: 0 },
      { viewport, seatCount: 6, viewer: 0 },
      { viewport: { width: 390, height: 844 }, seatCount: 3, viewer: 0, sides },
    ]) {
      const slim = planTable({ ...input, handSlim: true })
      expect(slim.regions).toEqual(planTable(input).regions)
      expect(slim.regions.handSlim).toBeUndefined()
    }
    expect(regionSizes({ width: 390, height: 844 }, false, true).hand).toBeGreaterThan(HAND_SLIM_PX - 1)
  })
})
