// Goal: the layout of a hand of 1 to 20 cards in docks from 230 to 1500 px wide and 96 to 200 px high keeps every card between 44 and 112 px wide in the shape of a card, shows at least 62 % of every card but the last (a 44 px strip under a touch screen), never leaves the line wider than the dock unless it says it scrolls, shrinks the cards before it scrolls and never grows them with more cards, and lifts a card by 40 % of its height.
import { describe, expect, it } from 'vitest'
import { CARD_ASPECT, FIT_LIMITS } from './fit'
import { HAND_LIFT, HAND_MIN_SHOWN, HAND_TOUCH_PX, handCardWidth, handLayout } from './handFit'

const WIDTHS = [230, 300, 400, 518, 700, 998, 1500]
const HEIGHTS = [96, 112, 122, 153, 200]
const COUNTS = Array.from({ length: 20 }, (_, index) => index + 1)

/** The width of the line of `count` cards of this layout. */
const lineOf = (count: number, box: number, gap: number): number => box + (count - 1) * (box + gap)

describe.each([false, true])('handLayout (coarse pointer %s)', (coarse) => {
  it('keeps the cards in the shape and the limits of a card, for 1 to 20 cards in every dock', () => {
    for (const width of WIDTHS) {
      for (const height of HEIGHTS) {
        for (const count of COUNTS) {
          const where = `${count} cards in ${width}x${height}`
          const { box } = handLayout({ width, height, count, coarse })
          expect(box.width, where).toBeGreaterThanOrEqual(FIT_LIMITS.minWidth)
          expect(box.width, where).toBeLessThanOrEqual(handCardWidth(height))
          expect(box.height, where).toBe(Math.round(box.width * CARD_ASPECT))
        }
      }
    }
  })

  it('shows at least 62 % of every card but the last, and 44 px of it under a touch screen', () => {
    for (const width of WIDTHS) {
      for (const height of HEIGHTS) {
        for (const count of COUNTS.slice(1)) {
          const where = `${count} cards in ${width}x${height}`
          const { box, gap } = handLayout({ width, height, count, coarse })
          const shown = box.width + gap
          expect(shown, where).toBeGreaterThanOrEqual(Math.ceil(box.width * HAND_MIN_SHOWN) - 1e-9)
          if (coarse) expect(shown, where).toBeGreaterThanOrEqual(Math.min(box.width, HAND_TOUCH_PX))
        }
      }
    }
  })

  it('never leaves the line wider than the dock unless it scrolls, and then it is wider', () => {
    for (const width of WIDTHS) {
      for (const height of HEIGHTS) {
        for (const count of COUNTS) {
          const where = `${count} cards in ${width}x${height}`
          const { box, gap, scrolls } = handLayout({ width, height, count, coarse })
          const line = lineOf(count, box.width, gap)
          if (scrolls) expect(line, where).toBeGreaterThan(width)
          else expect(line, where).toBeLessThanOrEqual(Math.max(width, box.width))
        }
      }
    }
  })

  it('never makes a card larger for more cards, and a gap only tighter', () => {
    for (const width of WIDTHS) {
      for (const height of HEIGHTS) {
        let card = Number.POSITIVE_INFINITY
        for (const count of COUNTS) {
          const { box } = handLayout({ width, height, count, coarse })
          expect(box.width, `${count} cards in ${width}x${height}`).toBeLessThanOrEqual(card)
          card = box.width
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

describe('handLayout compared with the cards of the dock', () => {
  it('uses the full card the dock allows while the hand fits, side by side', () => {
    const layout = handLayout({ width: 998, height: 122, count: 7 })
    expect(layout.box.width).toBe(handCardWidth(122))
    expect(layout.gap).toBe(FIT_LIMITS.gap)
    expect(layout.scrolls).toBe(false)
  })

  it('shrinks the cards to avoid a scroll on a mouse, down to the floor, and scrolls only past it', () => {
    const fits = handLayout({ width: 518, height: 122, count: 12 })
    expect(fits.scrolls).toBe(false)
    expect(fits.box.width).toBeLessThan(handCardWidth(122))
    const past = handLayout({ width: 230, height: 122, count: 20 })
    expect(past.scrolls).toBe(true)
    expect(past.box.width).toBe(FIT_LIMITS.minWidth)
  })

  it('keeps the card size under a touch screen and scrolls instead', () => {
    const touch = handLayout({ width: 300, height: 122, count: 12, coarse: true })
    expect(touch.scrolls).toBe(true)
    expect(touch.box.width).toBe(handCardWidth(122))
    expect(touch.box.width + touch.gap).toBeGreaterThanOrEqual(HAND_TOUCH_PX)
  })

  it('is total for an empty hand and for a dock with no room', () => {
    expect(handLayout({ width: 500, height: 120, count: 0 }).scrolls).toBe(false)
    expect(handLayout({ width: 0, height: 0, count: 3 }).box.width).toBe(FIT_LIMITS.minWidth)
    expect(handLayout({ width: Number.NaN, height: Number.NaN, count: 3 }).box.width).toBe(
      FIT_LIMITS.minWidth,
    )
  })
})
