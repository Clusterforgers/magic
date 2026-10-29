// Goal: the widest card a dock of this height holds in one line is its height over the shape of a card (5:7), between 44 and 112 px, for every height from 0 to 260 px.
import { describe, expect, it } from 'vitest'
import { CARD_ASPECT, FIT_LIMITS } from './fit'
import { handCardWidth } from './handFit'

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
