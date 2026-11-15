// Goal: the card preview keeps clear of the open prompt's strip and the docks: at the windows of the table it docks beside the strip or above it, a phone's takes the free stretch above the strip, and where nothing is clear it stands where `previewFit` puts it.
import { describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import type { Anchor, Viewport } from './preview'
import { fitClear, panelBox, SQUEEZED_HEIGHT_PX } from './previewClear'
import type { Box } from './previewClear'
import { PICTURE_FLOOR, previewFit } from './previewFit'

const rect = (left: number, top: number, right: number, bottom: number): Anchor => ({
  left,
  top,
  width: right - left,
  height: bottom - top,
})

const meets = (a: Box, b: Anchor): boolean =>
  a.left < b.left + b.width && a.right > b.left && a.top < b.top + b.height && a.bottom > b.top

const touches = (fit: ReturnType<typeof fitClear>, view: Viewport, clear: readonly Anchor[]): boolean =>
  clear.some((box) => meets(panelBox(fit, view), box))

/** What a Delves table measured with a choice of four cards open and one of them chosen (the strip, the hand dock, the controls). */
const TABLES: [string, Viewport, Anchor, Anchor[]][] = [
  [
    'a wide desktop',
    { width: 1830, height: 855 },
    rect(210, 344, 1486, 488),
    [rect(184, 208, 1518, 550), rect(0, 718, 1006, 855), rect(1006, 718, 1526, 855)],
  ],
  [
    'a laptop',
    { width: 1400, height: 900 },
    rect(210, 373, 1094, 524),
    [rect(184, 237, 1120, 594), rect(0, 756, 859, 900), rect(859, 756, 1128, 900)],
  ],
  [
    'a small laptop',
    { width: 1280, height: 720 },
    rect(210, 316, 968, 416),
    [rect(184, 180, 1000, 468), rect(0, 605, 739, 720), rect(739, 605, 1008, 720)],
  ],
]

describe('fitClear beside a prompt strip', () => {
  it.each(TABLES)('on %s docks beside the strip and covers none of it', (_, view, grid, clear) => {
    const fit = fitClear(grid, view, clear)
    expect(touches(fit, view, clear)).toBe(false)
    expect(touches(previewFit(grid, view), view, clear)).toBe(true)
    expect(fit.dock).toBe('right')
    expect(fit.picture).toBeGreaterThanOrEqual(PICTURE_FLOOR.narrow)
  })

  it.each(TABLES)('on %s leaves the strip alone when the cards sit on its right', (_, view, grid, clear) => {
    // The cards in the right half send the preview to the left, where the sentence and the buttons are.
    const strip = must(clear[0])
    const right = rect(
      strip.left + strip.width * 0.55,
      grid.top,
      strip.left + strip.width - 20,
      grid.top + grid.height,
    )
    const fit = fitClear(right, view, clear)
    expect(previewFit(right, view).dock).toBe('left')
    expect(touches(fit, view, clear)).toBe(false)
    expect(fit.dock).toBe('right')
  })

  it('goes above a strip that spans the window when there is room over it', () => {
    const view = { width: 1600, height: 900 }
    const clear = [rect(100, 520, 1500, 700), rect(0, 760, 1600, 900)]
    const fit = fitClear(rect(600, 560, 1000, 660), view, clear)
    expect(touches(fit, view, clear)).toBe(false)
    expect(fit.top).toBe(64)
    expect(fit.maxHeight).toBe(520 - 8 - 64)
    expect(fit.picture).toBeGreaterThanOrEqual(PICTURE_FLOOR.narrow)
  })

  it('keeps the panel above the controls of a short window and shows the old picture at least', () => {
    const view = { width: 1280, height: 480 }
    const clear = [rect(0, 376, 1280, 480)]
    const fit = fitClear(rect(100, 200, 200, 340), view, clear)
    expect(touches(fit, view, clear)).toBe(false)
    expect(fit.maxHeight).toBeGreaterThanOrEqual(SQUEEZED_HEIGHT_PX)
  })

  it('stands where previewFit puts it when no room is clear', () => {
    const view = { width: 1024, height: 768 }
    const grid = rect(210, 297, 940, 416)
    const clear = [rect(184, 161, 972, 468)]
    expect(fitClear(grid, view, clear)).toEqual(previewFit(grid, view))
  })

  it('is previewFit when nothing is kept clear, and ignores a box without size', () => {
    const view = { width: 1830, height: 855 }
    const grid = rect(210, 344, 1486, 488)
    expect(fitClear(grid, view, [])).toEqual(previewFit(grid, view))
    expect(fitClear(grid, view, [rect(300, 300, 300, 700), rect(0, 0, 0, 0)])).toEqual(previewFit(grid, view))
  })

  it('leaves a panel that already clears the strip as it is', () => {
    const view = { width: 1830, height: 855 }
    const grid = rect(210, 344, 1486, 488)
    expect(fitClear(grid, view, [rect(184, 208, 1400, 550)])).toEqual(previewFit(grid, view))
  })

  it('never touches a strip over a sweep of windows and strips, or falls back to previewFit', () => {
    for (let width = 1024; width <= 1920; width += 128) {
      for (let height = 600; height <= 1080; height += 120) {
        for (const reach of [0.5, 0.6, 0.7]) {
          const strip = rect(184, height * 0.25, Math.round(width * reach) + 300, height * 0.65)
          const grid = rect(210, height * 0.4, strip.left + strip.width - 30, height * 0.55)
          const clear = [strip, rect(0, height - 137, width - 300, height)]
          const view = { width, height }
          const fit = fitClear(grid, view, clear)
          const room = width - 12 - (strip.left + strip.width + 8)
          const touched = touches(fit, view, clear)
          if (room >= PICTURE_FLOOR.narrow + 48) expect(touched).toBe(false)
          if (touched) expect(fit).toEqual(previewFit(grid, view))
        }
      }
    }
  })
})

describe('fitClear on a phone', () => {
  const view = { width: 390, height: 844 }

  it('puts the panel in the stretch above the strip, never over its sentence and buttons', () => {
    const grid = rect(25, 66, 365, 481)
    const clear = [rect(8, 497, 382, 709), rect(0, 709, 230, 844), rect(230, 709, 390, 844)]
    const fit = fitClear(grid, view, clear)
    expect(touches(fit, view, clear)).toBe(false)
    expect(touches(previewFit(grid, view), view, clear)).toBe(true)
    expect(fit).toMatchObject({ dock: 'bottom', bottom: 844 - 489 })
    expect(panelBox(fit, view).top).toBeGreaterThanOrEqual(56)
  })

  it('docks at the top when the cards are low and the strip is above them', () => {
    const grid = rect(25, 500, 365, 640)
    const clear = [rect(8, 60, 382, 300), rect(0, 709, 390, 844)]
    const fit = fitClear(grid, view, clear)
    expect(touches(fit, view, clear)).toBe(false)
    expect(fit).toMatchObject({ dock: 'top', top: 308 })
  })

  it('stands where previewFit puts it when the strip fills the window', () => {
    const grid = rect(25, 66, 365, 300)
    const clear = [rect(0, 56, 390, 701)]
    expect(fitClear(grid, view, clear)).toEqual(previewFit(grid, view))
  })
})

describe('fitClear on a short narrow window', () => {
  it('keeps the old picture above the strip', () => {
    const view = { width: 620, height: 460 }
    const grid = rect(100, 100, 500, 330)
    const clear = [rect(8, 340, 612, 396), rect(0, 396, 620, 460)]
    const fit = fitClear(grid, view, clear)
    expect(touches(fit, view, clear)).toBe(false)
    expect(fit).toMatchObject({ picture: PICTURE_FLOOR.narrow, beside: true })
  })
})
