// Goal: the layout of a hand of 1 to 24 cards in docks from 230 to 1500 px wide and 96 to 200 px high keeps every card in the shape of a card and not under 68 px unless the dock is lower, puts the hand in one row of whole cards while they fit, then in two or three balanced rows of which each lies over the one before it so that the top 54 px (name and cost) of every card shows, never leaves a row wider than the dock or the rows taller than it, scrolls only when three rows at 68 px do not hold the hand (a touch screen keeping the full card size), and lifts a card by 40 % of its height.
import { describe, expect, it } from 'vitest'
import { CARD_ASPECT, FIT_LIMITS } from './fit'
import {
  HAND_FLOOR_PX,
  HAND_GAP_PX,
  HAND_HEAD_PX,
  HAND_LIFT,
  HAND_MAX_ROWS,
  handCardWidth,
  handLayout,
} from './handFit'

const WIDTHS = [230, 300, 400, 518, 602, 700, 998, 1500]
const HEIGHTS = [96, 112, 122, 153, 200]
const COUNTS = Array.from({ length: 24 }, (_, index) => index + 1)

/** The width of a row of `perRow` cards. */
const rowOf = (perRow: number, box: number, gap: number): number => perRow * box + (perRow - 1) * gap

describe.each([false, true])('handLayout (coarse pointer %s)', (coarse) => {
  it('keeps the cards in the shape of a card, between the floor and what the dock is tall', () => {
    for (const width of WIDTHS) {
      for (const height of HEIGHTS) {
        for (const count of COUNTS) {
          const where = `${count} cards in ${width}x${height}`
          const { box } = handLayout({ width, height, count, coarse })
          expect(box.width, where).toBeGreaterThanOrEqual(Math.min(handCardWidth(height), HAND_FLOOR_PX))
          expect(box.width, where).toBeLessThanOrEqual(handCardWidth(height))
          expect(box.height, where).toBe(Math.round(box.width * CARD_ASPECT))
        }
      }
    }
  })

  it('fits every row in the dock and every row in the height, unless it scrolls', () => {
    for (const width of WIDTHS) {
      for (const height of HEIGHTS) {
        for (const count of COUNTS) {
          const where = `${count} cards in ${width}x${height}`
          const { box, gap, rows, perRow, overlap, scrolls } = handLayout({ width, height, count, coarse })
          expect(rows * perRow, where).toBeGreaterThanOrEqual(count)
          expect((rows - 1) * perRow, where).toBeLessThan(count)
          if (scrolls) {
            expect(rows, where).toBe(1)
            expect(rowOf(count, box.width, gap), where).toBeGreaterThan(width)
          } else {
            expect(rowOf(perRow, box.width, gap), where).toBeLessThanOrEqual(width)
            expect(box.height + (rows - 1) * (box.height - overlap), where).toBeLessThanOrEqual(
              Math.max(height, box.height),
            )
          }
        }
      }
    }
  })

  it('shows the top 54 px of every card another row lies over, and nothing is over a single row', () => {
    for (const width of WIDTHS) {
      for (const height of HEIGHTS) {
        for (const count of COUNTS) {
          const where = `${count} cards in ${width}x${height}`
          const { box, rows, overlap } = handLayout({ width, height, count, coarse })
          if (rows === 1) expect(overlap, where).toBe(0)
          else expect(box.height - overlap, where).toBe(Math.min(box.height, HAND_HEAD_PX))
          expect(rows, where).toBeLessThanOrEqual(HAND_MAX_ROWS)
        }
      }
    }
  })

  it('takes the fewest rows: another row only when one row of the smallest card does not hold the hand', () => {
    for (const width of WIDTHS) {
      for (const height of HEIGHTS) {
        for (const count of COUNTS) {
          const where = `${count} cards in ${width}x${height}`
          const { rows } = handLayout({ width, height, count, coarse })
          const floor = Math.min(handCardWidth(height), HAND_FLOOR_PX)
          const oneRow = rowOf(count, floor, HAND_GAP_PX) <= width
          if (oneRow) expect(rows, where).toBe(1)
        }
      }
    }
  })

  it('never makes a card smaller than a card of a hand that holds fewer in the same rows', () => {
    for (const width of WIDTHS) {
      for (const height of HEIGHTS) {
        const seen = new Map<number, number>()
        for (const count of COUNTS) {
          const { box, rows, scrolls } = handLayout({ width, height, count, coarse })
          if (scrolls) continue
          const before = seen.get(rows) ?? Number.POSITIVE_INFINITY
          expect(box.width, `${count} cards in ${width}x${height}`).toBeLessThanOrEqual(before)
          seen.set(rows, box.width)
        }
      }
    }
  })

  it('lifts a card by 40 % of its height, which is room inside the fan above the cards', () => {
    expect(HAND_LIFT).toBe(0.4)
    for (const width of WIDTHS) {
      for (const count of [1, 7, 20]) {
        const { box, lift } = handLayout({ width, height: 120, count, coarse })
        expect(lift).toBe(Math.round(box.height * 0.4))
        expect(lift).toBeGreaterThan(0)
        expect(lift).toBeLessThan(box.height)
      }
    }
  })
})

describe('handLayout at the table of 1600x1000 (a dock of 602x153 for the cards)', () => {
  it.each([
    [1, 1, 1, 109],
    [7, 1, 7, 81],
    [8, 1, 8, 71],
    [9, 2, 5, 71],
    [14, 2, 7, 71],
    [16, 2, 8, 71],
  ])('draws %i cards in %i row(s) of %i, %i px wide', (count, rows, perRow, cardWidth) => {
    const layout = handLayout({ width: 602, height: 153, count })
    expect(layout).toMatchObject({ rows, perRow, scrolls: false })
    expect(layout.box.width).toBe(cardWidth)
  })

  it('scrolls a hand of 17 cards in a row of the smallest card, since three rows are taller than the dock', () => {
    const layout = handLayout({ width: 602, height: 153, count: 17 })
    expect(layout).toMatchObject({ rows: 1, scrolls: true })
    expect(layout.box.width).toBe(HAND_FLOOR_PX)
  })

  it('wraps into three rows in a dock tall enough for them', () => {
    const layout = handLayout({ width: 602, height: 230, count: 20 })
    expect(layout).toMatchObject({ rows: 3, perRow: 7, scrolls: false })
  })
})

describe('handLayout compared with the dock', () => {
  it('uses the full card the dock allows while the hand fits, side by side', () => {
    const layout = handLayout({ width: 998, height: 122, count: 7 })
    expect(layout.box.width).toBe(handCardWidth(122))
    expect(layout.gap).toBe(HAND_GAP_PX)
    expect(layout).toMatchObject({ rows: 1, overlap: 0, scrolls: false })
  })

  it('keeps the card size under a touch screen and scrolls instead of shrinking', () => {
    const touch = handLayout({ width: 214, height: 94, count: 7, coarse: true })
    expect(touch.scrolls).toBe(true)
    expect(touch.box.width).toBe(handCardWidth(94))
    const mouse = handLayout({ width: 214, height: 94, count: 7 })
    expect(mouse.scrolls).toBe(true)
    expect(mouse.box.width).toBe(Math.min(handCardWidth(94), HAND_FLOOR_PX))
  })

  it('is total for an empty hand and for a dock with no room', () => {
    expect(handLayout({ width: 500, height: 120, count: 0 }).scrolls).toBe(false)
    expect(handLayout({ width: 0, height: 0, count: 3 }).box.width).toBe(FIT_LIMITS.minWidth)
    expect(handLayout({ width: Number.NaN, height: Number.NaN, count: 3 }).box.width).toBe(
      FIT_LIMITS.minWidth,
    )
  })
})
