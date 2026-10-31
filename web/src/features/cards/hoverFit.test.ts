// Goal: the hover picture of the builder grows from its old width to twice that as far as the window's height and the room beside the hovered element allow, stands on the side with room, and stays inside the window.
import { describe, expect, it } from 'vitest'
import { hoverFit, HOVER_CEILING_PX, HOVER_FLOOR_PX } from './hoverFit'

describe('hoverFit', () => {
  const view = { width: 1440, height: 900 }

  it.each([
    // [element box, picture width, side of the element it stands on]
    [{ left: 100, right: 300, top: 400 }, HOVER_CEILING_PX, 'right'],
    [{ left: 1100, right: 1300, top: 400 }, 488, 'left'],
    [{ left: 700, right: 760, top: 400 }, 488, 'right'],
  ] as const)('stands the picture beside %j at %i px on its %s', (element, width, side) => {
    const fit = hoverFit(element, view)
    expect(fit.width).toBe(width)
    if (side === 'right') expect(fit.left).toBe(element.right + 12)
    else expect(fit.left + fit.width).toBe(element.left - 12)
  })

  it.each([
    [1920, 1080, 488],
    [1440, 900, 488],
    [1366, 768, 488],
    [1280, 720, 488],
    [1024, 600, 417],
    [800, 400, 274],
    [800, 300, HOVER_FLOOR_PX],
  ])('draws the picture inside a %ix%i window at %i px wide', (w, h, width) => {
    const fit = hoverFit({ left: 20, right: 120, top: 100 }, { width: w, height: h })
    expect(fit.width).toBe(width)
    expect(fit.top).toBeGreaterThanOrEqual(8)
    if (width > HOVER_FLOOR_PX) expect(fit.top + (fit.width * 7) / 5).toBeLessThanOrEqual(h - 8)
  })

  it('takes the side with more room when neither holds the old width, and never leaves the left edge', () => {
    const fit = hoverFit({ left: 90, right: 340, top: 10 }, { width: 400, height: 900 })
    expect(fit.width).toBe(HOVER_FLOOR_PX)
    expect(fit.left).toBe(8)
  })
})
