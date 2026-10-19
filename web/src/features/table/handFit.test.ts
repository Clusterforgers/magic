// Goal: the hand dock draws the widest card its height allows (between 44 and 112 px, 5:7), keeps the normal gap while the hand fits its width, overlaps just enough to fit, never shows less than 62 % of a card, and scrolls sideways only past that, for 0 to 12 cards in docks from 160 to 1200 px wide and 96 to 200 px high.
import { describe, expect, it } from 'vitest'
import { CARD_ASPECT, FIT_LIMITS } from './fit'
import { HAND_MIN_SHOWN, handCardWidth, handFan } from './handFit'

describe('handCardWidth', () => {
  it.each([
    [96, 68],
    [112, 80],
    [122, 87],
    [153, 109],
    [157, 112],
    [200, 112],
    [60, 44],
    [0, 44],
    [Number.NaN, 44],
  ])('is %i px of height: a card %i px wide', (height, width) => {
    expect(handCardWidth(height)).toBe(width)
  })

  it('never makes a card taller than the dock, over every height from 60 to 260 px', () => {
    for (let height = 60; height <= 260; height += 1) {
      const width = handCardWidth(height)
      expect(width).toBeGreaterThanOrEqual(FIT_LIMITS.minWidth)
      expect(width).toBeLessThanOrEqual(FIT_LIMITS.maxWidth)
      // The floor is the exception: a dock under 62 px cannot hold a card of 44 px.
      if (width > FIT_LIMITS.minWidth) expect(Math.round(width * CARD_ASPECT)).toBeLessThanOrEqual(height)
    }
  })
})

describe('handFan', () => {
  it('keeps the normal gap while the hand fits', () => {
    expect(handFan(600, 7, 78)).toEqual({ gap: 6, scrolls: false })
    expect(handFan(7 * 78 + 6 * 6, 7, 78)).toEqual({ gap: 6, scrolls: false })
  })

  it('tightens the gap, then overlaps, by just what is missing', () => {
    // 7 cards of 78 px with 6 px gaps need 582 px; in 580 the step between cards is 83 px, so the gap is 5.
    expect(handFan(580, 7, 78)).toEqual({ gap: 5, scrolls: false })
    // 258 px: the step is (258 - 70) / 6 = 31 px, which is 44 % of a 70 px card: under the least, so it scrolls.
    expect(handFan(258, 7, 70)).toEqual({ gap: Math.ceil(70 * HAND_MIN_SHOWN) - 70, scrolls: true })
    // 400 px: the step is (400 - 70) / 6 = 55 px, 79 % of the card.
    expect(handFan(400, 7, 70)).toEqual({ gap: 55 - 70, scrolls: false })
  })

  it.each([0, 1])('has nothing to overlap with %i cards', (count) => {
    expect(handFan(100, count, 80)).toEqual({ gap: 6, scrolls: false })
  })

  it('never shows less than 62 % of a card and scrolls only when it must', () => {
    for (let width = 160; width <= 1200; width += 20) {
      for (let count = 0; count <= 12; count += 1) {
        for (let height = 96; height <= 200; height += 26) {
          const cardWidth = handCardWidth(height)
          const fan = handFan(width, count, cardWidth)
          const where = `${count} cards of ${cardWidth} px in ${width} px`
          if (count > 1)
            expect(cardWidth + fan.gap, where).toBeGreaterThanOrEqual(cardWidth * HAND_MIN_SHOWN - 1e-9)
          const line = cardWidth + Math.max(0, count - 1) * (cardWidth + fan.gap)
          if (!fan.scrolls) expect(line, where).toBeLessThanOrEqual(Math.max(width, cardWidth) + 1e-9)
          else expect(line, where).toBeGreaterThan(width)
        }
      }
    }
  })

  it('overlaps more as the dock narrows and never less', () => {
    for (const count of [3, 7, 12]) {
      let last = Number.POSITIVE_INFINITY
      for (let width = 1200; width >= 160; width -= 10) {
        const { gap } = handFan(width, count, 78)
        expect(gap).toBeLessThanOrEqual(last)
        last = gap
      }
    }
  })
})
