//! Goal: the board rows split by what each board holds, so the duel board draws large cards and no board scrolls, and the opponents keep their pods and a row of cards under a prompt sheet.
import { describe, expect, it } from 'vitest'
import { fixtureState } from './dev/devGame'
import {
  BOARD_KEEP_PX,
  boardCounts,
  boardFrames,
  MIN_ROW_SHARE,
  FLOOR_OWN_CARD_PX,
  podColumnFloor,
  shareSplit,
  splitBoards,
  splitRegions,
  THEIR_SCALE,
  theirChromeOf,
  withSplit,
} from './boardSplit'
import { POD_CRAMPED_PX } from './cornerFit'
import { buildFixture } from './dev/fixture'
import { LANE_LIMITS, laneHeight } from './laneFit'
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

  it('keeps an empty board its least share below the strips of seats that left', () => {
    const own = counts(10, 14, 8)
    const plain = splitBoards({ ...base, room: 700, own, theirs: [NONE] })
    const led = splitBoards({ ...base, room: 700, theirChrome: 24 + 64, theirLead: 64, own, theirs: [NONE] })
    expect(plain.opponents).toBeLessThan(700 * MIN_ROW_SHARE + 8)
    expect(led.opponents).toBeGreaterThanOrEqual(700 * MIN_ROW_SHARE + 64)
  })

  it.each([
    ['an empty opponent', counts(3, 4, 2), [NONE], 900],
    ['two sparse opponents', counts(2, 2, 1), [counts(1, 1, 0), NONE], 1000],
    ['an empty board facing an empty board of yours but lands', counts(5, 0, 0), [NONE], 800],
  ])('shares the room neither board can use evenly: %s', (_, own, theirs, room) => {
    const split = splitBoards({ ...base, room, own, theirs })
    expect(split.ownCard).toBeGreaterThan(0)
    const ownSpare =
      split.own -
      Math.max(room * MIN_ROW_SHARE, base.ownChrome + laneHeight(base.ownWidth, own, split.ownCard))
    const theirNeed = Math.max(...theirs.map((each) => laneHeight(base.theirWidth, each, split.theirCard)))
    const theirSpare = split.opponents - Math.max(room * MIN_ROW_SHARE, base.theirChrome + theirNeed)
    expect(ownSpare).toBeGreaterThan(8)
    expect(ownSpare).toBeCloseTo(theirSpare, 6)
    expect(split.opponents).toBeGreaterThan(room * MIN_ROW_SHARE)
  })
})

// The band's room above the strip is kept in every case (`Regions.sheetRoom`); a smaller reserve changes nothing.
const VIEWPORTS = [
  [1920, 1080, 160, 130, 0],
  [1920, 1080, 160, 130, 40],
  [1440, 900, 120, 110, 0],
  [1440, 900, 120, 110, 56],
  [1366, 768, 100, 88, 0],
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
  // The band's room is kept above the strip whatever the prompt (`Regions.sheetRoom`).
  const kept = Math.max(reserve, table.regions.sheetRoom)
  const theirFrame = { ...frames.theirs, chrome: frames.theirs.chrome + kept }
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
      // No higher than the band, whose room the split keeps free of cards.
      const band = regions.shared + regions.sheetRoom
      expect(regions.sheet).toBeCloseTo(Math.min(band, height * 0.4, regions.shared + room), 6)
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
        Math.min(
          regions.shared + regions.sheetRoom,
          height * 0.4,
          regions.shared + 320 - chrome - BOARD_KEEP_PX,
        ),
        6,
      )
    },
  )
})

describe("a phone's sheet and the boss's head", () => {
  // The boss fight on a phone (`/dev/table?seats=4&boss=0&bare=1&prompt=mulligan`): the abilities bar and the boss's
  // pod reach 190 px down the opponents' area, and the opening hand's sheet wants 224 px over the strip.
  const viewport = { width: 390, height: 844 }
  const table = planTable({
    viewport,
    seatCount: 4,
    viewer: 0,
    sides: { boss: 3, enemies: [3] },
  })
  const state = fixtureState(buildFixture({ seats: 4 }))
  const counts = { ...boardCounts(state, table.seats), allies: [] as LaneCounts[] }

  it.each([
    ['a sheet of answers', 'sheet'],
    ['a sheet of cards', 'sheetWide'],
  ] as const)("leaves the boss's bar and pod clear of %s, with the sheet's room kept", (_name, key) => {
    const head = 190
    const overhang = 224
    const regions = splitRegions(table.regions, table.seats, counts, 64, head, overhang)
    const kept = theirChromeOf(boardFrames(table.regions, table.seats).theirs.chrome, head)
    expect(kept).toBeGreaterThan(SHEET_KEEP_PX)
    const over = regions[key] - regions.shared
    expect(regions.opponents - over).toBeGreaterThanOrEqual(kept - 1e-9)
    // The band keeps the room it had where the head was no taller than the usual keep; a sheet that grows gives way to the head.
    const low = splitRegions(table.regions, table.seats, counts, 64, 0, overhang)
    if (key === 'sheet') expect(over).toBeGreaterThanOrEqual(low[key] - low.shared - 1e-9)
    expect(regions.own).toBeGreaterThanOrEqual((regions.opponents + regions.own) * MIN_ROW_SHARE - 1e-9)
  })

  it('leaves a phone of tiles as it was: its head is under the usual keep', () => {
    const plain = planTable({ viewport, seatCount: 4, viewer: 0 })
    const flat = boardCounts(state, plain.seats)
    const without = splitRegions(plain.regions, plain.seats, flat, 64, 0, 0)
    const withSheet = splitRegions(plain.regions, plain.seats, flat, 64, 0, 224)
    expect(withSheet.opponents).toBe(without.opponents)
    expect(withSheet.sheetWide).toBe(without.sheetWide)
  })

  it("moves no row of the boss's phone table for a sheet: the band's room is kept with or without one", () => {
    const without = splitRegions(table.regions, table.seats, counts, 0, 190, 0)
    const withSheet = splitRegions(table.regions, table.seats, counts, 0, 190, 224)
    expect(withSheet.opponents).toBe(without.opponents)
    expect(without.sheet - without.shared).toBeLessThanOrEqual(without.sheetRoom + 1e-9)
  })
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
    expect(regions.opponents - (regions.sheetWide - regions.shared)).toBeGreaterThanOrEqual(
      192 + BOARD_KEEP_PX - 1e-9,
    )
    // The band is the strip; a sheet that grows (a decision with no column for its choices) still has room.
    expect(regions.sheet).toBe(regions.shared)
    expect(regions.sheetWide).toBeGreaterThan(regions.shared + 32)
  })
})

describe('the pod column of the opponent', () => {
  // The duel board at 1366x768 under a priority prompt with three stack items, whose sheet wants 76 px over the row.
  it.each([
    [1366, 768, 76],
    [1440, 900, 64],
    [1920, 1080, 76],
  ])('keeps the column at %ix%i unfolded above a sheet that wants %i px', (width, height, overhang) => {
    const table = planTable({ viewport: { width, height }, seatCount: 2, viewer: 0 })
    const state = fixtureState(buildFixture({ seats: 2, board: 'duel' }))
    const regions = splitRegions(table.regions, table.seats, boardCounts(state, table.seats), 64, 0, overhang)
    const pads = boardFrames(table.regions, table.seats).theirs.chrome
    expect(regions.opponents - pads - overhang).toBeGreaterThanOrEqual(POD_CRAMPED_PX)
    expect(regions.own).toBeGreaterThanOrEqual((regions.opponents + regions.own) * MIN_ROW_SHARE)
  })

  it('asks for no floor on a phone or where the opponents stand in columns', () => {
    const phone = planTable({ viewport: { width: 390, height: 844 }, seatCount: 2, viewer: 0 })
    const columns = planTable({ viewport: { width: 1440, height: 900 }, seatCount: 4, viewer: 0 })
    expect(podColumnFloor(phone.regions, phone.seats, 80)).toBe(0)
    expect(podColumnFloor(columns.regions, columns.seats, 80)).toBe(0)
  })

  it.each([0, 200, 280, 1000])(
    'lifts the opponents to a floor of %i px only while your creatures keep FLOOR_OWN_CARD_PX, else by what your lanes leave',
    (floor) => {
      const input = {
        room: 520,
        ownChrome: 24,
        theirChrome: 24,
        ownWidth: 900,
        theirWidth: 900,
        own: { lands: 4, creatures: 2, other: 0 },
        theirs: [{ lands: 4, creatures: 2, other: 0 }],
      }
      const plain = splitBoards(input)
      const split = splitBoards({ ...input, theirFloor: floor })
      expect(split.opponents + split.own).toBeCloseTo(520)
      expect(split.own).toBeGreaterThanOrEqual(520 * MIN_ROW_SHARE)
      expect(split.opponents).toBeGreaterThanOrEqual(plain.opponents)
      if (split.ownCard === plain.ownCard) return
      // Your creatures gave way for the floor: no further than FLOOR_OWN_CARD_PX, and the floor holds.
      expect(split.ownCard).toBeGreaterThanOrEqual(FLOOR_OWN_CARD_PX)
      expect(split.opponents).toBeGreaterThanOrEqual(floor)
    },
  )
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

  it.each([
    // Two rows of frames: the lower row's zone counts from its own frame, moved up to the first row.
    [
      44,
      [
        { top: 150, frame: 52 },
        { top: 450, frame: 352 },
      ],
      106,
    ],
    [
      44,
      [
        { top: 150, frame: 52 },
        { top: 470, frame: 352 },
      ],
      126,
    ],
    [44, [{ top: 150, frame: Number.NaN }], 106],
  ])('of an area at %i px over framed zones %o is %i px', (top, zones, head) => {
    expect(headOf(top, zones)).toBe(head)
  })
})

describe('the split of a grid of opponents', () => {
  const base = { room: 900, ownChrome: 24, theirChrome: 80, ownWidth: 1400, theirWidth: 500 }
  const theirs = [counts(6, 5, 3), counts(6, 5, 3), counts(6, 5, 3), counts(6, 5, 3)]
  it.each([1, 2, 3])('gives %i rows of boards each their lanes and pod line', (rows) => {
    const split = splitBoards({ ...base, own: counts(8, 6, 4), theirs, theirRows: rows, theirRowChrome: 80 })
    const need = laneHeight(base.theirWidth, theirs[0] ?? NONE, split.theirCard)
    expect(split.opponents).toBeGreaterThanOrEqual(rows * (need + base.theirChrome))
  })
  it('gives opponents that cannot fit at the floor what they need there before your row grows', () => {
    const crowded = Array.from({ length: 6 }, () => counts(14, 12, 10))
    const split = splitBoards({
      ...base,
      own: counts(4, 3, 2),
      theirs: crowded,
      theirRows: 2,
      theirRowChrome: 80,
    })
    const ownFloor = laneHeight(base.ownWidth, counts(4, 3, 2), LANE_LIMITS.minWidth) + base.ownChrome
    const ownLeast = Math.max(base.room * MIN_ROW_SHARE, ownFloor)
    const theirFloor = 2 * laneHeight(base.theirWidth, crowded[0] ?? NONE, LANE_LIMITS.minWidth) + 2 * 80
    expect(split.opponents).toBeCloseTo(Math.min(base.room - ownLeast, theirFloor))
    expect(split.opponents).toBeGreaterThan(shareSplit(base.room).opponents)
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
