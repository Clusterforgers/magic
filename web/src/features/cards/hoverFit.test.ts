// Goal: the hover preview outside the table docks at the side of the window away from the hovered element, below the top bar, as the table's preview does, and its picture grows to the ceiling as far as the window's height allows, never below the floor and never wider than the window.
import { describe, expect, it } from 'vitest'
import {
  hoverFit,
  HOVER_CEILING_PX,
  HOVER_EDGE_PX,
  HOVER_FLOOR_PX,
  HOVER_PADDING_PX,
  HOVER_TEXT_PX,
  HOVER_TOP_PX,
} from './hoverFit'

describe('hoverFit', () => {
  const view = { width: 1440, height: 900 }

  it.each([
    // [element box, dock]
    [{ left: 100, right: 300, top: 400 }, 'right'],
    [{ left: 1100, right: 1300, top: 400 }, 'left'],
    [{ left: 700, right: 760, top: 400 }, 'left'],
  ] as const)('docks the panel for %j at the %s side of the window', (element, dock) => {
    const fit = hoverFit(element, view)
    expect(fit.dock).toBe(dock)
    expect(fit.width).toBe(HOVER_CEILING_PX)
    expect(fit.top).toBe(HOVER_TOP_PX)
    const right = fit.left + fit.width + HOVER_PADDING_PX
    if (dock === 'right') expect(right).toBe(view.width - HOVER_EDGE_PX)
    else expect(fit.left).toBe(HOVER_EDGE_PX)
  })

  it.each([
    [1920, 1080, 480],
    [1440, 900, 480],
    [1366, 768, 397],
    [1280, 720, 362],
    [1024, 600, 277],
    [800, 400, HOVER_FLOOR_PX],
  ])('draws the picture inside a %ix%i window at %i px wide', (w, h, width) => {
    const fit = hoverFit({ left: 20, right: 120, top: 100 }, { width: w, height: h })
    expect(fit.width).toBe(width)
    expect(fit.maxHeight).toBe(h - HOVER_TOP_PX - HOVER_EDGE_PX)
    if (width > HOVER_FLOOR_PX)
      expect(HOVER_PADDING_PX + (fit.width * 7) / 5 + HOVER_TEXT_PX).toBeLessThanOrEqual(fit.maxHeight)
  })

  it.each([
    [400, HOVER_FLOOR_PX],
    [250, 250 - HOVER_EDGE_PX * 2 - HOVER_PADDING_PX],
  ])('keeps the panel inside a window %i px wide', (w, width) => {
    const fit = hoverFit({ left: 90, right: 340, top: 10 }, { width: w, height: 900 })
    expect(fit.width).toBe(width)
    expect(fit.left).toBeGreaterThanOrEqual(HOVER_EDGE_PX)
    expect(fit.left + fit.width + HOVER_PADDING_PX).toBeLessThanOrEqual(w - HOVER_EDGE_PX)
  })
})
