// Goal: the table's regions follow the window: the rows always add up to the window height with the minimums of the design (44 px bar, 56 to 88 px shared strip, 112 to 200 px hand dock), the side area is a column, a rail or a sheet by width, the CSS templates equal the numbers, and the mode and seat arrangement follow the seat count and the width of a board column (grid for 2 to 4 seats unless a column would be under 280 px, focus otherwise, the menu overriding), and in focus mode the strip of tiles leads with the seat whose turn it is.
import { describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import {
  activeFirst,
  arrangeSeats,
  breakpointOf,
  chooseMode,
  clampedHeight,
  gridColumns,
  gridColumnWidth,
  gridRows,
  HAND_CLAMP,
  HAND_PHONE_CLAMP,
  mainColumns,
  MIN_COLUMN_PX,
  planTable,
  regionSizes,
  SHARED_CLAMP,
  STOPS_ROW_MIN_WIDTH_PX,
} from './layout'
import type { Breakpoint, TableMode, Viewport } from './layout'

/** The sizes the design names, plus a tablet and a phone on its side. */
const SIZES: ReadonlyArray<[string, Viewport]> = [
  ['1920x1080', { width: 1920, height: 1080 }],
  ['1440x900', { width: 1440, height: 900 }],
  ['1366x768', { width: 1366, height: 768 }],
  ['1280x720', { width: 1280, height: 720 }],
  ['1024x768', { width: 1024, height: 768 }],
  ['390x844', { width: 390, height: 844 }],
  ['768x1024', { width: 768, height: 1024 }],
  ['844x390', { width: 844, height: 390 }],
]

const SEAT_COUNTS = [2, 3, 4, 5, 6, 8, 12, 13, 24, 64]

describe('breakpointOf', () => {
  it.each<[number, Breakpoint]>([
    [0, 'xs'],
    [390, 'xs'],
    [639, 'xs'],
    [640, 's'],
    [1023, 's'],
    [1024, 'm'],
    [1279, 'm'],
    [1280, 'l'],
    [1599, 'l'],
    [1600, 'xl'],
    [3840, 'xl'],
  ])('puts a window %i px wide in %s', (width, expected) => {
    expect(breakpointOf(width)).toBe(expected)
  })
})

describe('regionSizes', () => {
  it.each(SIZES)('adds the rows up to the height of a %s window', (_name, viewport) => {
    const regions = regionSizes(viewport)
    const total = regions.topBar + regions.opponents + regions.shared + regions.own + regions.hand
    expect(total).toBeCloseTo(viewport.height, 6)
    expect(regions.main.width + regions.sideWidth).toBe(viewport.width)
  })

  it.each([
    ['1920x1080', 'xl', 'column', 320, 44, 88, 183.6],
    ['1440x900', 'l', 'column', 316.8, 44, 81, 153],
    ['1366x768', 'l', 'column', 300.52, 44, 69.12, 130.56],
    ['1280x720', 'l', 'column', 281.6, 44, 64.8, 122.4],
    ['1024x768', 'm', 'rail', 44, 44, 69.12, 130.56],
    ['390x844', 'xs', 'sheet', 0, 44, 75.96, 118.16],
    ['768x1024', 's', 'sheet', 0, 44, 88, 174.08],
    ['844x390', 's', 'sheet', 0, 32, 56, 112],
  ])(
    'sizes a %s window: %s, side %s %i px, bar %i, shared %f, hand %f',
    (name, breakpoint, side, sideWidth, bar, shared, hand) => {
      const regions = regionSizes(SIZES.find(([label]) => label === name)?.[1] ?? { width: 0, height: 0 })
      expect(regions.breakpoint).toBe(breakpoint)
      expect(regions.side).toBe(side)
      expect(regions.sideWidth).toBeCloseTo(Number(sideWidth), 2)
      expect(regions.topBar).toBe(bar)
      expect(regions.shared).toBeCloseTo(Number(shared), 2)
      expect(regions.hand).toBeCloseTo(Number(hand), 2)
    },
  )

  it('gives the opponents 1 part and the own board 1.15 parts of what is left', () => {
    for (const [, viewport] of SIZES) {
      const { opponents, own } = regionSizes(viewport)
      expect(own / opponents).toBeCloseTo(1.15, 6)
    }
  })

  it('keeps every row inside its clamp for any window from 320x400 to 2560x1440', () => {
    for (let width = 320; width <= 2560; width += 80) {
      for (let height = 400; height <= 1440; height += 40) {
        const regions = regionSizes({ width, height })
        const where = `${width}x${height}`
        expect(regions.shared, where).toBeGreaterThanOrEqual(SHARED_CLAMP.min)
        expect(regions.shared, where).toBeLessThanOrEqual(SHARED_CLAMP.max)
        const hand = regions.phone ? HAND_PHONE_CLAMP : HAND_CLAMP
        expect(regions.hand, where).toBeGreaterThanOrEqual(hand.min)
        expect(regions.hand, where).toBeLessThanOrEqual(hand.max)
        expect(regions.opponents, where).toBeGreaterThan(0)
        expect(regions.own, where).toBeGreaterThan(regions.opponents)
        expect(regions.topBar + regions.opponents + regions.shared + regions.own + regions.hand).toBeCloseTo(
          height,
          6,
        )
        // A long prompt grows to 40 % of the height, up over the opponents and no further than the top bar.
        expect(regions.sheet, where).toBeLessThanOrEqual(height * 0.4 + 1e-9)
        expect(regions.sheet, where).toBeLessThanOrEqual(regions.shared + regions.opponents + 1e-9)
      }
    }
  })

  it.each([
    ['1920x1080', 432],
    ['1280x720', 288],
    ['390x844', 337.6],
    // A window so low that the strip and the opponents are less than 40 %: the sheet takes them and no more.
    ['844x390', 56 + (390 - 32 - 56 - 112) / 2.15],
  ])('lets the sheet of a %s window grow %f px up over the opponents', (name, most) => {
    const viewport = must(SIZES.find(([label]) => label === name)?.[1])
    expect(regionSizes(viewport).sheet).toBeCloseTo(most, 6)
  })

  it('draws a column of 280 to 320 px from L, a rail of 44 px at M and a sheet below', () => {
    for (let width = 320; width <= 2560; width += 16) {
      const regions = regionSizes({ width, height: 900 })
      if (regions.side === 'column') {
        expect(width, `${width}`).toBeGreaterThanOrEqual(1280)
        expect(regions.sideWidth, `${width}`).toBeGreaterThanOrEqual(280)
        expect(regions.sideWidth, `${width}`).toBeLessThanOrEqual(320)
      } else if (regions.side === 'rail') {
        expect(width).toBeGreaterThanOrEqual(1024)
        expect(width).toBeLessThan(1280)
        expect(regions.sideWidth).toBe(44)
      } else {
        expect(width).toBeLessThan(1024)
        expect(regions.sideWidth).toBe(0)
      }
    }
  })

  it.each([
    ['390x844', true, false, 44],
    ['844x390', false, true, 32],
    ['1280x720', false, false, 44],
  ])(
    'knows a phone from a wide window: %s (phone %s, landscape %s, bar %i)',
    (name, phone, landscape, bar) => {
      const regions = regionSizes(SIZES.find(([label]) => label === name)?.[1] ?? { width: 0, height: 0 })
      expect(regions.phone).toBe(phone)
      expect(regions.landscape).toBe(landscape)
      expect(regions.topBar).toBe(bar)
    },
  )

  it.each([
    [Number.NaN, 900],
    [1280, Number.NaN],
    [-5, -5],
    [0, 0],
    [Number.POSITIVE_INFINITY, 900],
  ])('never throws and never goes negative for a window of %s by %s', (width, height) => {
    const regions = regionSizes({ width, height })
    for (const value of [regions.opponents, regions.own, regions.shared, regions.hand, regions.sheet]) {
      expect(Number.isFinite(value)).toBe(true)
      expect(value).toBeGreaterThanOrEqual(0)
    }
  })
})

/** Reads `clamp(<min>px, <vh>dvh, <max>px)` back into numbers, so the CSS can be held to the code. */
function readClamp(css: string): { min: number; vh: number; max: number } {
  const match = /^clamp\((\d+)px, (\d+)dvh, (\d+)px\)$/.exec(css)
  if (match === null) throw new Error(`not a clamp: ${css}`)
  return { min: Number(match[1]), vh: Number(match[2]), max: Number(match[3]) }
}

describe('the CSS templates', () => {
  it('writes the rows the design names for a wide window', () => {
    expect(gridRows({ phone: false, landscape: false })).toBe(
      '44px minmax(0, 1fr) clamp(56px, 9dvh, 88px) minmax(0, 1.15fr) clamp(112px, 17dvh, 200px)',
    )
  })

  it('writes a 32 px bar for a phone on its side and a 96 to 120 px hand dock for a phone upright', () => {
    expect(gridRows({ phone: false, landscape: true })).toMatch(/^32px /)
    expect(gridRows({ phone: true, landscape: false })).toMatch(/clamp\(96px, 14dvh, 120px\)$/)
  })

  it.each(SIZES)('gives the sizes regionSizes does at %s', (_name, viewport) => {
    const regions = regionSizes(viewport)
    expect(gridRows(regions)).toMatch(new RegExp(`^${regions.topBar}px `))
    const [shared, hand] = gridRows(regions).match(/clamp\([^)]*\)/g) ?? []
    expect(clampedHeight(readClamp(shared ?? ''), viewport.height)).toBeCloseTo(regions.shared, 9)
    expect(clampedHeight(readClamp(hand ?? ''), viewport.height)).toBeCloseTo(regions.hand, 9)
  })

  it('writes the columns of the main area and the side column, rail or sheet', () => {
    expect(gridColumns('column')).toBe('minmax(0, 1fr) clamp(280px, 22vw, 320px)')
    expect(gridColumns('rail')).toBe('minmax(0, 1fr) 44px')
    expect(gridColumns('sheet')).toBe('minmax(0, 1fr)')
  })
})

describe('gridColumnWidth', () => {
  it.each([
    // One opponent: the main area less its padding, the 168 px pod column and the gap.
    [2, 998, 998 - 16 - 168 - 8],
    // Two opponents: two columns; three or more: three columns.
    [3, 998, (998 - 16 - 8) / 2],
    [4, 998, (998 - 16 - 16) / 3],
    [6, 998, (998 - 16 - 16) / 3],
  ])('is the narrowest column of %i seats in a main area %i px wide', (seats, main, expected) => {
    expect(gridColumnWidth(seats, main)).toBeCloseTo(expected, 6)
  })

  it('shrinks as the window does', () => {
    for (const seats of SEAT_COUNTS) {
      let last = Number.POSITIVE_INFINITY
      for (let width = 1600; width >= 320; width -= 40) {
        const column = gridColumnWidth(seats, width)
        expect(column).toBeLessThanOrEqual(last)
        last = column
      }
    }
  })
})

describe('chooseMode', () => {
  // The seat counts that draw a grid in each window, the others draw focus.
  const GRID: Record<string, number[]> = {
    '1920x1080': [2, 3, 4],
    '1440x900': [2, 3, 4],
    '1366x768': [2, 3, 4],
    '1280x720': [2, 3, 4],
    '1024x768': [2, 3, 4],
    '390x844': [],
    '768x1024': [2, 3],
    '844x390': [2, 3],
  }

  it.each(SIZES)('picks the mode of every table size in a %s window', (name, viewport) => {
    for (const seats of SEAT_COUNTS) {
      const plan = planTable({ viewport, seatCount: seats, viewer: 0 })
      const expected: TableMode = (GRID[name] ?? []).includes(seats) ? 'grid' : 'focus'
      expect(plan.seats.mode, `${seats} seats`).toBe(expected)
    }
  })

  it('draws focus from 5 seats whatever the width', () => {
    for (const seatCount of [5, 6, 12, 13, 24, 64]) {
      expect(chooseMode({ seatCount, mainWidth: 5000 })).toBe('focus')
    }
  })

  it('draws focus when a column would be under 280 px, and grid at exactly 280', () => {
    // Two columns of 3 seats are (main - 24) / 2 wide.
    expect(chooseMode({ seatCount: 3, mainWidth: 2 * MIN_COLUMN_PX + 24 })).toBe('grid')
    expect(chooseMode({ seatCount: 3, mainWidth: 2 * MIN_COLUMN_PX + 23 })).toBe('focus')
  })

  it.each<[number, TableMode, TableMode]>([
    [2, 'focus', 'focus'],
    [2, 'grid', 'grid'],
    [6, 'grid', 'grid'],
    [12, 'grid', 'grid'],
  ])(
    'lets the menu override: %i seats asked for %s give %s even in a narrow window',
    (seatCount, asked, got) => {
      expect(chooseMode({ seatCount, mainWidth: 300, override: asked })).toBe(got)
      expect(chooseMode({ seatCount, mainWidth: 3000, override: asked })).toBe(got)
    },
  )
})

describe('the seat shape', () => {
  it.each([
    [2, 'grid', 'duel'],
    [3, 'grid', 'columns'],
    [4, 'grid', 'columns'],
    [6, 'grid', 'columns'],
    [2, 'focus', 'focus'],
    [6, 'focus', 'focus'],
  ] as const)('arranges %i seats in %s mode as %s', (seatCount, mode, shape) => {
    expect(arrangeSeats({ seatCount, viewer: 0, mode }).shape).toBe(shape)
  })
})

describe('planTable', () => {
  it.each([2, 3, 4, 8, 12, 13, 24, 40, 64])(
    'plans a table of %i seats: a tile for every opponent, the active one first, compact from 8',
    (seatCount) => {
      const viewport = { width: 1920, height: 1080 }
      const active = seatCount - 1
      const plan = planTable({ viewport, seatCount, viewer: 0, active })
      expect(plan.seats.mode).toBe(seatCount <= 4 ? 'grid' : 'focus')
      expect(plan.seats.compact).toBe(seatCount >= 8)
      expect(plan.seats.opponents).toHaveLength(seatCount - 1)
      if (plan.seats.mode === 'focus') {
        expect(plan.seats.tiles).toHaveLength(seatCount - 1)
        expect(new Set(plan.seats.tiles).size).toBe(seatCount - 1)
        expect(plan.seats.tiles[0]).toBe(active)
        expect(plan.seats.boards).toHaveLength(1)
      } else {
        expect(plan.seats.boards).toEqual(plan.seats.opponents)
      }
    },
  )

  it.each(SIZES)('puts the pod beside the board except on a phone: %s', (_name, viewport) => {
    const plan = planTable({ viewport, seatCount: 2, viewer: 0 })
    expect(plan.pod).toBe(viewport.width < 640 ? 'line' : 'aside')
  })

  it('draws the stop ladder as one row in the controls dock from the XL breakpoint up, at any height', () => {
    for (const [, viewport] of SIZES) {
      const plan = planTable({ viewport, seatCount: 2, viewer: 0 })
      expect(plan.stopsRow).toBe(viewport.width >= STOPS_ROW_MIN_WIDTH_PX)
    }
    expect(STOPS_ROW_MIN_WIDTH_PX).toBe(1600)
  })

  it.each([
    [1920, 1080, true],
    [1600, 1000, true],
    [1440, 900, false],
    [1366, 768, false],
    [1280, 720, false],
    [1280, 480, false],
    // One pixel short of XL: the same content opens from the dock's "Stops" button.
    [1599, 1000, false],
    [1024, 768, false],
    [768, 1024, false],
    [844, 390, false],
    [390, 844, false],
  ])('draws the ladder of a %ix%i window as a row: %s', (width, height, row) => {
    expect(planTable({ viewport: { width, height }, seatCount: 2, viewer: 0 }).stopsRow).toBe(row)
  })

  it('makes the controls dock wider while it holds the ladder row, and a column on a phone', () => {
    expect(mainColumns({ phone: false }, false)).toBe('minmax(0, 1fr) fit-content(480px)')
    expect(mainColumns({ phone: false }, true)).toBe('minmax(0, 1fr) fit-content(760px)')
    expect(mainColumns({ phone: true }, false)).toBe('minmax(0, 1fr) fit-content(160px)')
    expect(mainColumns({ phone: true }, true)).toBe('minmax(0, 1fr) fit-content(160px)')
  })

  it.each([
    ['a phone', { width: 390, height: 844 }],
    ['a tablet', { width: 820, height: 1180 }],
    ['1280x720', { width: 1280, height: 720 }],
    ['1920x1080', { width: 1920, height: 1080 }],
  ])('puts the seat whose turn it is first in the strip, on %s', (_name, viewport) => {
    const on = (active: number | null) => planTable({ viewport, seatCount: 5, viewer: 0, active }).seats.tiles
    expect(on(3)).toEqual([3, 1, 2, 4])
    expect(on(1)).toEqual([1, 2, 3, 4])
    expect(on(4)).toEqual([4, 1, 2, 3])
    // The viewer's own turn leads nothing: the tiles are the opponents' only, in turn order.
    expect(on(0)).toEqual([1, 2, 3, 4])
    expect(on(null)).toEqual([1, 2, 3, 4])
  })

  it('leads the strip with the active seat whichever opponent the board shows, and keeps the focus where it is', () => {
    const viewport = { width: 1440, height: 900 }
    const plan = planTable({ viewport, seatCount: 6, viewer: 0, mode: 'focus', focus: 2, active: 4 })
    expect(plan.seats.tiles).toEqual([4, 1, 2, 3, 5])
    expect(plan.seats.focused).toBe(2)
    expect(plan.seats.boards).toEqual([2])
  })

  it('draws no tiles in grid mode, so no seat leads anything', () => {
    const plan = planTable({ viewport: { width: 1920, height: 1080 }, seatCount: 4, viewer: 0, active: 2 })
    expect(plan.seats.mode).toBe('grid')
    expect(plan.seats.tiles).toEqual([])
    expect(plan.seats.boards).toEqual([1, 2, 3])
  })

  it.each([
    [[1, 2, 3, 4], 3, [3, 1, 2, 4]],
    [[1, 2, 3, 4], 1, [1, 2, 3, 4]],
    [[1, 2, 3, 4], 0, [1, 2, 3, 4]],
    [[1, 2, 3, 4], null, [1, 2, 3, 4]],
    [[1, 2, 3, 4], undefined, [1, 2, 3, 4]],
    [[], 2, []],
  ])('moves the active seat to the front of %j: %s gives %j', (tiles, active, expected) => {
    expect(activeFirst(tiles, active)).toEqual(expected)
  })

  it('follows the chosen mode and the focus through to the seats', () => {
    const viewport = { width: 1440, height: 900 }
    const plan = planTable({ viewport, seatCount: 4, viewer: 1, mode: 'focus', focus: 3 })
    expect(plan.seats.mode).toBe('focus')
    expect(plan.seats.focused).toBe(3)
    expect(plan.seats.tiles).toEqual([2, 3, 0])
  })
})
