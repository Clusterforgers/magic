// Goal: fitRows picks, for every box from 280x120 to 1700x520 and every spread of 0 to 80 stacks over 1 to 3 rows, the largest card width from 112 px down to the 44 px floor (steps of 2) at which all rows wrap inside the box, or the floor with a "+N" count when the zone must scroll; its numbers equal a line-by-line wrap in whole px, a bigger box never gets a smaller card, and empty or degenerate input neither hangs nor throws.
import { describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import { CARD_ASPECT, FIT_LIMITS, LAND_SCALE, fitRows } from './fit'
import type { Fit, FitBox, FitGroup, FitLimits } from './fit'

/** A small seeded generator, so a failing case can be replayed. */
function mulberry32(seed: number): () => number {
  let state = seed
  return () => {
    state = (state + 0x6d2b79f5) | 0
    let t = Math.imul(state ^ (state >>> 15), 1 | state)
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

const range = (from: number, to: number, step: number): number[] => {
  const values: number[] = []
  for (let value = from; value <= to; value += step) values.push(value)
  return values
}

/** The browser's wrap, one box at a time: a box that does not fit the rest of the line starts the next one. */
function wrapLines(stacks: number, boxWidth: number, width: number, gap: number): number[] {
  const lines: number[] = []
  let used = 0
  for (let index = 0; index < stacks; index += 1) {
    const last = lines.length - 1
    if (last >= 0 && used + gap + width <= boxWidth) {
      lines[last] = (lines[last] ?? 0) + 1
      used += gap + width
    } else {
      lines.push(1)
      used = width
    }
  }
  return lines
}

interface Laid {
  rows: { width: number; height: number; lines: number[] }[]
  height: number
}

/** The rows at one card width, laid out box by box with the documented rounding. */
function lay(box: FitBox, groups: readonly FitGroup[], limits: FitLimits, cardWidth: number): Laid {
  const rows = groups.map((group) => {
    const width = Math.round(cardWidth * (group.scale ?? 1))
    const height = Math.round(width * (group.aspect ?? CARD_ASPECT))
    return { width, height, lines: wrapLines(group.stacks, Math.floor(box.width), width, limits.gap) }
  })
  const heights = rows.flatMap((row) => row.lines.map(() => row.height))
  const total = heights.reduce((sum, height) => sum + height, 0)
  return { rows, height: total + Math.max(0, heights.length - 1) * limits.rowGap }
}

/** The widths the engine may answer with, largest first: from the maximum down by the step, then the floor. */
function ladder(limits: FitLimits): number[] {
  const sizes: number[] = []
  for (let width = limits.maxWidth; width > limits.minWidth; width -= limits.step) sizes.push(width)
  sizes.push(limits.minWidth)
  return sizes
}

/** Runs the engine and holds every number of its answer against the box-by-box layout. */
function expectSound(box: FitBox, groups: readonly FitGroup[], limits: FitLimits, label: string): Fit {
  const fit = fitRows(box, groups, limits)
  const sizes = ladder(limits)
  const laid = lay(box, groups, limits, fit.cardWidth)
  const placed = groups.reduce((sum, group) => sum + group.stacks, 0)

  expect(sizes, label).toContain(fit.cardWidth)
  expect(
    fit.layout.map((row) => [row.stacks, row.width, row.height, row.lines]),
    label,
  ).toEqual(laid.rows.map((row, index) => [groups[index]?.stacks, row.width, row.height, row.lines.length]))
  for (const [index, row] of fit.layout.entries()) {
    const first = laid.rows[index]?.lines[0]
    if (row.lines > 1) expect(row.perLine, label).toBe(first)
  }
  expect(fit.height, label).toBe(laid.height)
  expect(fit.lines, label).toBe(laid.rows.reduce((sum, row) => sum + row.lines.length, 0))
  expect(fit.rows, label).toBe(laid.rows.filter((row) => row.lines.length > 0).length)

  if (fit.scrolls) {
    expect(fit.cardWidth, label).toBe(limits.minWidth)
    expect(laid.height, label).toBeGreaterThan(box.height)
    let top = 0
    let hidden = 0
    for (const row of laid.rows) {
      for (const onLine of row.lines) {
        if (top + row.height > box.height) hidden += onLine
        top += row.height + limits.rowGap
      }
    }
    expect(fit.hidden, label).toBe(hidden)
    expect(fit.hidden, label).toBeGreaterThan(0)
    expect(fit.hidden, label).toBeLessThanOrEqual(placed)
  } else {
    expect(laid.height, label).toBeLessThanOrEqual(box.height)
    expect(fit.hidden, label).toBe(0)
    // Largest: no bigger size on the ladder fits.
    for (const bigger of sizes.slice(0, sizes.indexOf(fit.cardWidth))) {
      expect(lay(box, groups, limits, bigger).height, `${label} at ${bigger}`).toBeGreaterThan(box.height)
    }
  }
  return fit
}

/** `count` stacks over `rows` rows: creatures, then other permanents, then lands at `LAND_SCALE`. */
function spread(count: number, rows: 1 | 2 | 3): FitGroup[] {
  const shares = rows === 1 ? [1] : rows === 2 ? [0.6, 0.4] : [0.4, 0.2, 0.4]
  let left = count
  return shares.map((share, index) => {
    const stacks = index === shares.length - 1 ? left : Math.round(count * share)
    left -= stacks
    return index > 0 && index === shares.length - 1 ? { stacks, scale: LAND_SCALE } : { stacks }
  })
}

const BOXES: FitBox[] = [
  { width: 280, height: 120 },
  { width: 360, height: 160 },
  { width: 480, height: 200 },
  { width: 640, height: 240 },
  { width: 800, height: 300 },
  { width: 1000, height: 340 },
  { width: 1200, height: 400 },
  { width: 1400, height: 440 },
  { width: 1600, height: 500 },
  { width: 1700, height: 520 },
]
const COUNTS = [0, 8, 24, 48, 80]
const ROW_COUNTS = [1, 2, 3] as const

const CASES = BOXES.flatMap((box) =>
  COUNTS.flatMap((count) =>
    ROW_COUNTS.map((rows) => ({
      name: `${box.width}x${box.height}, ${count} stacks in ${rows} ${rows === 1 ? 'row' : 'rows'}`,
      box,
      count,
      groups: spread(count, rows),
    })),
  ),
)

describe('fitRows over 150 boxes, counts and rows', () => {
  it('has 150 cases from 280x120 to 1700x520 that place exactly the stacks they name', () => {
    expect(CASES).toHaveLength(150)
    expect(CASES[0]?.box).toEqual({ width: 280, height: 120 })
    expect(CASES.at(-1)?.box).toEqual({ width: 1700, height: 520 })
    for (const { groups, count } of CASES) {
      expect(groups.reduce((sum, group) => sum + group.stacks, 0)).toBe(count)
    }
  })

  it.each(CASES)('$name', ({ box, groups }) => {
    // Fits or scrolls, from 44 to 112 px, equal to the box-by-box wrap, the largest that fits.
    const fit = expectSound(box, groups, FIT_LIMITS, 'table case')
    expect(fit.cardWidth).toBeGreaterThanOrEqual(FIT_LIMITS.minWidth)
    expect(fit.cardWidth).toBeLessThanOrEqual(FIT_LIMITS.maxWidth)
    if (!fit.scrolls) expect(fit.height).toBeLessThanOrEqual(box.height)
    // A bigger box never gives a smaller card, whichever way it grows.
    const grown: FitBox[] = [
      { width: box.width + 1, height: box.height },
      { width: box.width, height: box.height + 1 },
      { width: box.width + 120, height: box.height },
      { width: box.width, height: box.height + 60 },
      { width: box.width + 120, height: box.height + 60 },
      { width: 4000, height: 4000 },
    ]
    for (const bigger of grown) {
      expect(fitRows(bigger, groups).cardWidth).toBeGreaterThanOrEqual(fit.cardWidth)
    }
  })

  it('reaches every outcome: the largest card, a size between, the floor that fits and a scrolling zone', () => {
    const fits = CASES.map(({ box, groups }) => fitRows(box, groups))
    const { maxWidth, minWidth } = FIT_LIMITS
    const kinds = {
      largest: fits.filter((fit) => fit.cardWidth === maxWidth),
      between: fits.filter((fit) => !fit.scrolls && fit.cardWidth > minWidth && fit.cardWidth < maxWidth),
      floor: fits.filter((fit) => !fit.scrolls && fit.cardWidth === minWidth),
      scrolling: fits.filter((fit) => fit.scrolls),
    }
    for (const [kind, found] of Object.entries(kinds)) expect(found.length, kind).toBeGreaterThanOrEqual(3)
  })

  it('scrolls the smallest box with the most stacks and never the biggest box', () => {
    const smallest = must(CASES.find((entry) => entry.box.width === 280 && entry.count === 80))
    const biggest = CASES.filter((entry) => entry.box.width === 1700)
    expect(fitRows(smallest.box, smallest.groups).scrolls).toBe(true)
    for (const entry of biggest) expect(fitRows(entry.box, entry.groups).scrolls, entry.name).toBe(false)
  })
})

describe('worked examples', () => {
  it('takes the largest card that is no taller than the box (one stack, 280x120)', () => {
    const fit = fitRows({ width: 280, height: 120 }, [{ stacks: 1 }])
    // 86 px wide is 120.4 px high, which rounds to 120; 88 px would be 123.
    expect(fit).toEqual({
      cardWidth: 86,
      layout: [{ stacks: 1, width: 86, height: 120, perLine: 3, lines: 1 }],
      rows: 1,
      lines: 1,
      height: 120,
      scrolls: false,
      hidden: 0,
    })
  })

  it('wraps by width: ten boxes of 94 px and nine gaps of 6 are 994 px, and 993 px wraps them', () => {
    const groups = [{ stacks: 10 }]
    expect(fitRows({ width: 1000, height: 200 }, groups).cardWidth).toBe(94)
    expect(fitRows({ width: 994, height: 200 }, groups).cardWidth).toBe(94)
    // At 993 px only nine boxes of 94 fit a line; two lines are 2 * 132 + 4 = 268 high, over 200, so 92 px it is.
    const narrower = fitRows({ width: 993, height: 200 }, groups)
    expect(narrower.cardWidth).toBe(92)
    expect(narrower.layout[0]).toMatchObject({ width: 92, height: 129, perLine: 10, lines: 1 })
  })

  it('wraps by height: six stacks in 400 px take two lines of four, as tall as the box allows', () => {
    const groups = [{ stacks: 6 }]
    // 88 px boxes are 123 high and four fit a line (4 * 88 + 3 * 6 = 370): two lines and a gap are 250.
    const exact = fitRows({ width: 400, height: 250 }, groups)
    expect(exact.cardWidth).toBe(88)
    expect(exact.layout[0]).toEqual({ stacks: 6, width: 88, height: 123, perLine: 4, lines: 2 })
    expect(exact.height).toBe(250)
    // One pixel less and 88 px no longer fits; 86 px boxes are 120 high.
    const short = fitRows({ width: 400, height: 249 }, groups)
    expect(short.cardWidth).toBe(86)
    expect(short.height).toBe(2 * 120 + 4)
  })

  it('draws lands at 0.8 of the width with the same 5:7 shape, rounded to whole px', () => {
    const fit = fitRows({ width: 5000, height: 5000 }, [{ stacks: 3 }, { stacks: 3, scale: LAND_SCALE }])
    // 112 * 0.8 is 89.6, which is 90 px; 90 * 1.4 is 126.
    expect(fit.cardWidth).toBe(112)
    expect(fit.layout.map((row) => [row.width, row.height])).toEqual([
      [112, 157],
      [90, 126],
    ])
  })

  it('rounds halves up and keeps aspects and scales exact to the pixel', () => {
    const limits = { ...FIT_LIMITS, maxWidth: 45, minWidth: 45 }
    const fit = fitRows({ width: 5000, height: 5000 }, [{ stacks: 1, scale: 0.5, aspect: 2 }], limits)
    // 45 * 0.5 is 22.5, which is 23; 23 * 2 is 46.
    expect(fit.layout[0]).toMatchObject({ width: 23, height: 46 })
  })

  it('puts the row gap between every two lines, wrapped or of different rows, and none around an empty row', () => {
    const wide = { width: 5000, height: 5000 }
    const one = fitRows(wide, [{ stacks: 2 }])
    const two = fitRows(wide, [{ stacks: 2 }, { stacks: 2 }])
    const gapped = fitRows(wide, [{ stacks: 2 }, { stacks: 0 }, { stacks: 2 }])
    expect(one.height).toBe(157)
    expect(two.height).toBe(157 + FIT_LIMITS.rowGap + 157)
    expect(gapped.height).toBe(two.height)
    expect(gapped.rows).toBe(2)
    expect(gapped.layout[1]).toMatchObject({ stacks: 0, lines: 0 })
    const tall = fitRows({ width: 280, height: 5000 }, [{ stacks: 12 }], {
      ...FIT_LIMITS,
      maxWidth: 100,
      minWidth: 100,
    })
    // 100 px boxes with gaps of 6 fit two to a line in 280 px: six lines of 140 and five gaps of 4.
    expect(tall.layout[0]).toMatchObject({ perLine: 2, lines: 6, height: 140 })
    expect(tall.height).toBe(6 * 140 + 5 * FIT_LIMITS.rowGap)
  })

  it('does not depend on the order of the rows for the card width', () => {
    const rows: FitGroup[] = [{ stacks: 9 }, { stacks: 4 }, { stacks: 21, scale: LAND_SCALE }]
    const box = { width: 700, height: 260 }
    const sizes = [rows, [...rows].reverse(), [rows[1], rows[2], rows[0]]].map(
      (order) => fitRows(box, order as FitGroup[]).cardWidth,
    )
    expect(new Set(sizes).size).toBe(1)
  })
})

describe('a zone that scrolls', () => {
  it('stays at the 44 px floor and counts the stacks below the box (80 stacks, 280x120)', () => {
    const fit = fitRows({ width: 280, height: 120 }, [{ stacks: 80 }])
    // 44 px boxes are 62 high, five to a line, sixteen lines; only the first line ends inside 120 px.
    expect(fit).toEqual({
      cardWidth: 44,
      layout: [{ stacks: 80, width: 44, height: 62, perLine: 5, lines: 16 }],
      rows: 1,
      lines: 16,
      height: 16 * 62 + 15 * 4,
      scrolls: true,
      hidden: 75,
    })
  })

  it('counts a half-filled last line and the lines of a later row as hidden too', () => {
    // Lines of 5, 5 and 2 stacks: 62 + 4 + 62 = 128 ends inside 130 px, the third line does not.
    expect(fitRows({ width: 280, height: 130 }, [{ stacks: 12 }])).toMatchObject({ scrolls: true, hidden: 2 })
    // The second row starts below the first line plus a gap, at 66 px, and ends at 66 + 62 = 128.
    const rows = fitRows({ width: 280, height: 130 }, [{ stacks: 5 }, { stacks: 5 }, { stacks: 5 }])
    expect(rows).toMatchObject({ scrolls: true, hidden: 5 })
    expect(rows.layout.map((row) => row.lines)).toEqual([1, 1, 1])
  })

  it('hides every stack of a box with no height, and a line that is only partly inside counts as hidden', () => {
    expect(fitRows({ width: 400, height: 0 }, [{ stacks: 7 }])).toMatchObject({ scrolls: true, hidden: 7 })
    // One 44 px box is 62 high; a 61 px box cuts it.
    expect(fitRows({ width: 400, height: 61 }, [{ stacks: 1 }])).toMatchObject({ scrolls: true, hidden: 1 })
    expect(fitRows({ width: 400, height: 62 }, [{ stacks: 1 }])).toMatchObject({ scrolls: false, hidden: 0 })
  })

  it('does not scroll at the floor when everything fits there', () => {
    // 45 stacks at 44 px in a 280 px box: nine lines of five, 9 * 62 + 8 * 4 = 590.
    const fit = fitRows({ width: 280, height: 590 }, [{ stacks: 45 }])
    expect(fit).toMatchObject({ cardWidth: 44, scrolls: false, hidden: 0, height: 590 })
    expect(fitRows({ width: 280, height: 589 }, [{ stacks: 45 }])).toMatchObject({ scrolls: true, hidden: 5 })
  })
})

describe('nothing to place', () => {
  it.each([
    ['no rows', []],
    ['one empty row', [{ stacks: 0 }]],
    ['three empty rows', [{ stacks: 0 }, { stacks: 0, scale: LAND_SCALE }, { stacks: 0 }]],
  ] as [string, FitGroup[]][])(
    'takes the largest card in any box, even a box with no room (%s)',
    (_, groups) => {
      for (const box of [
        { width: 0, height: 0 },
        { width: 280, height: 120 },
        { width: 1700, height: 520 },
      ]) {
        const fit = fitRows(box, groups)
        expect(fit).toMatchObject({ cardWidth: 112, rows: 0, lines: 0, height: 0, scrolls: false, hidden: 0 })
      }
    },
  )
})

describe('limits', () => {
  it('are 112 px down to 44 in steps of 2, 6 px between boxes and 4 between lines, when none are given', () => {
    expect(FIT_LIMITS).toEqual({ maxWidth: 112, minWidth: 44, step: 2, gap: 6, rowGap: 4 })
    expect(CARD_ASPECT).toBe(7 / 5)
    expect(LAND_SCALE).toBe(0.8)
    const box = { width: 900, height: 280 }
    expect(fitRows(box, [{ stacks: 20 }])).toEqual(fitRows(box, [{ stacks: 20 }], FIT_LIMITS))
  })

  it('walks from the largest width down by the step and ends exactly on the floor', () => {
    const limits = { maxWidth: 50, minWidth: 44, step: 4, gap: 6, rowGap: 4 }
    const seen = new Set<number>()
    for (const height of range(0, 120, 1))
      seen.add(fitRows({ width: 500, height }, [{ stacks: 3 }], limits).cardWidth)
    expect([...seen].sort((a, b) => b - a)).toEqual([50, 46, 44])
  })

  it('uses custom widths, steps and gaps', () => {
    const limits = { maxWidth: 60, minWidth: 20, step: 5, gap: 2, rowGap: 1 }
    const fit = expectSound(
      { width: 100, height: 100 },
      [{ stacks: 9 }, { stacks: 4, scale: 0.5 }],
      limits,
      'custom',
    )
    expect(ladder(limits)).toContain(fit.cardWidth)
    expect((limits.maxWidth - fit.cardWidth) % limits.step).toBe(0)
  })

  it('rounds fractional limits and replaces a step under 1, so no input can hang the loop', () => {
    const limits = { maxWidth: 100.4, minWidth: 43.6, step: 0, gap: 6, rowGap: 4 }
    const fit = fitRows({ width: 1000, height: 0 }, [{ stacks: 1 }], limits)
    // The range is 100 down to 44 in steps of 1, and nothing fits a box with no height.
    expect(fit).toMatchObject({ cardWidth: 44, scrolls: true })
    expect(fitRows({ width: 1000, height: 5000 }, [{ stacks: 1 }], limits).cardWidth).toBe(100)
    expect(fitRows({ width: 1000, height: 5000 }, [{ stacks: 1 }], { ...limits, step: -3 }).cardWidth).toBe(
      100,
    )
  })

  it('accepts a floor above the largest width by treating them as one size', () => {
    const limits = { maxWidth: 10, minWidth: 40, step: 2, gap: 6, rowGap: 4 }
    expect(fitRows({ width: 500, height: 5000 }, [{ stacks: 3 }], limits).cardWidth).toBe(40)
    expect(fitRows({ width: 500, height: 1 }, [{ stacks: 3 }], limits)).toMatchObject({
      cardWidth: 40,
      scrolls: true,
    })
  })

  it('reads NaN and infinite limits as the table defaults', () => {
    const limits = {
      maxWidth: Number.NaN,
      minWidth: Number.POSITIVE_INFINITY,
      step: Number.NaN,
      gap: Number.NaN,
      rowGap: Number.NaN,
    }
    expect(fitRows({ width: 1000, height: 340 }, [{ stacks: 20 }], limits)).toEqual(
      fitRows({ width: 1000, height: 340 }, [{ stacks: 20 }], FIT_LIMITS),
    )
  })
})

describe('rounding and degenerate input', () => {
  it('floors a fractional box, since a fraction of a pixel is no room', () => {
    const groups = [{ stacks: 10 }]
    expect(fitRows({ width: 993.99, height: 200.9 }, groups)).toEqual(
      fitRows({ width: 993, height: 200 }, groups),
    )
    expect(fitRows({ width: 994.4, height: 200 }, groups).cardWidth).toBe(94)
  })

  it('reads a negative, NaN or infinite box as no room', () => {
    for (const box of [
      { width: -5, height: -5 },
      { width: Number.NaN, height: Number.NaN },
      { width: Number.POSITIVE_INFINITY, height: Number.NEGATIVE_INFINITY },
    ]) {
      expect(fitRows(box, [{ stacks: 4 }])).toMatchObject({ cardWidth: 44, scrolls: true, hidden: 4 })
    }
  })

  it('reads a negative, fractional or NaN stack count as a whole number of at least 0', () => {
    const box = { width: 1000, height: 300 }
    expect(fitRows(box, [{ stacks: -3 }])).toEqual(fitRows(box, [{ stacks: 0 }]))
    expect(fitRows(box, [{ stacks: Number.NaN }])).toEqual(fitRows(box, [{ stacks: 0 }]))
    expect(fitRows(box, [{ stacks: 7.9 }])).toEqual(fitRows(box, [{ stacks: 7 }]))
  })

  it('reads a bad scale or aspect as the default', () => {
    const box = { width: 1000, height: 300 }
    const expected = fitRows(box, [{ stacks: 9 }])
    for (const bad of [0, -1, Number.NaN, Number.POSITIVE_INFINITY]) {
      expect(fitRows(box, [{ stacks: 9, scale: bad, aspect: bad }])).toEqual(expected)
    }
  })

  it('still places one box a line in a box narrower than a card', () => {
    const fit = fitRows({ width: 30, height: 1000 }, [{ stacks: 4 }])
    expect(fit.layout[0]).toMatchObject({ perLine: 1, lines: 4 })
  })

  it('is deterministic and leaves its input alone', () => {
    const box = Object.freeze({ width: 900, height: 280 })
    const groups = Object.freeze([
      Object.freeze({ stacks: 13 }),
      Object.freeze({ stacks: 6, aspect: 1.5 }),
      Object.freeze({ stacks: 17, scale: LAND_SCALE }),
    ])
    const limits = Object.freeze({ ...FIT_LIMITS })
    const first = fitRows(box, groups, limits)
    expect(fitRows(box, groups, limits)).toEqual(first)
    expect(groups.map((group) => group.stacks)).toEqual([13, 6, 17])
    expect(limits).toEqual(FIT_LIMITS)
  })
})

describe('monotonic in the box and in the stacks', () => {
  const WIDTHS = range(200, 1700, 25)
  const HEIGHTS = range(60, 600, 15)
  const MIXES: [string, FitGroup[]][] = [
    ['one row of 30', [{ stacks: 30 }]],
    [
      'three rows of 14, 10 and 16 lands',
      [{ stacks: 14 }, { stacks: 10 }, { stacks: 16, scale: LAND_SCALE }],
    ],
    ['two rows of 40', [{ stacks: 40 }, { stacks: 40, scale: LAND_SCALE }]],
  ]

  it.each(MIXES)('never shrinks the card as the box grows wider or taller (%s)', (_, groups) => {
    const grid = HEIGHTS.map((height) => WIDTHS.map((width) => fitRows({ width, height }, groups).cardWidth))
    const at = (y: number, x: number): number => grid[y]?.[x] ?? Number.NaN
    for (let y = 0; y < HEIGHTS.length; y += 1) {
      for (let x = 0; x < WIDTHS.length; x += 1) {
        if (x > 0)
          expect(at(y, x), `${WIDTHS[x]}x${HEIGHTS[y]} against the box to its left`).toBeGreaterThanOrEqual(
            at(y, x - 1),
          )
        if (y > 0)
          expect(at(y, x), `${WIDTHS[x]}x${HEIGHTS[y]} against the box above`).toBeGreaterThanOrEqual(
            at(y - 1, x),
          )
      }
    }
  })

  it.each([
    [280, 120],
    [640, 240],
    [1700, 520],
  ])('never grows the card as stacks are added to a %i x %i box', (width, height) => {
    let before = Number.POSITIVE_INFINITY
    for (let stacks = 0; stacks <= 120; stacks += 1) {
      const size = fitRows({ width, height }, [
        { stacks },
        { stacks: Math.floor(stacks / 2), scale: LAND_SCALE },
      ]).cardWidth
      expect(size, `${stacks} stacks`).toBeLessThanOrEqual(before)
      before = size
    }
  })
})

describe('seeded random boxes, rows and limits', () => {
  it('stay sound, monotonic and independent of row order over 600 cases', () => {
    const random = mulberry32(20240607)
    const int = (low: number, high: number): number => low + Math.floor(random() * (high - low + 1))
    let scrolling = 0
    let largest = 0
    for (let index = 0; index < 600; index += 1) {
      const label = `random case ${index}`
      const box = { width: int(60, 1900), height: int(20, 700) }
      const groups: FitGroup[] = Array.from({ length: int(1, 4) }, () => ({
        stacks: int(0, 60),
        scale: random() < 0.4 ? LAND_SCALE : 1,
        aspect: random() < 0.8 ? CARD_ASPECT : 1 + random(),
      }))
      const limits =
        random() < 0.5
          ? FIT_LIMITS
          : {
              maxWidth: int(50, 150),
              minWidth: int(10, 50),
              step: int(1, 9),
              gap: int(0, 14),
              rowGap: int(0, 14),
            }
      const fit = expectSound(box, groups, limits, label)
      if (fit.scrolls) scrolling += 1
      if (fit.cardWidth === limits.maxWidth) largest += 1

      const bigger = { width: box.width + int(0, 500), height: box.height + int(0, 250) }
      expect(fitRows(bigger, groups, limits).cardWidth, `${label}: bigger box`).toBeGreaterThanOrEqual(
        fit.cardWidth,
      )
      const fewer = groups.map((group) => ({ ...group, stacks: int(0, group.stacks) }))
      expect(fitRows(box, fewer, limits).cardWidth, `${label}: fewer stacks`).toBeGreaterThanOrEqual(
        fit.cardWidth,
      )
      const shuffled = [...groups].sort(() => random() - 0.5)
      expect(fitRows(box, shuffled, limits).cardWidth, `${label}: row order`).toBe(fit.cardWidth)
    }
    // The generator reaches both kinds of answer, so the checks above are not all about one of them.
    expect(scrolling).toBeGreaterThan(50)
    expect(largest).toBeGreaterThan(50)
  })
})
