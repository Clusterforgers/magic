// Goal: a popup opened at any point of any window size lies fully inside the window, `margin` from each edge, and a popup larger than the window sits at the margin.
import { describe, expect, it } from 'vitest'
import { clampToView } from './popup'

const SIZE = { width: 288, height: 400 }
const WIDE = { width: 1400, height: 900 }

describe('clampToView', () => {
  it.each([
    { name: 'middle', at: { x: 100, y: 100 }, view: WIDE, want: { left: 100, top: 100 } },
    { name: 'right edge', at: { x: 1390, y: 100 }, view: WIDE, want: { left: 1104, top: 100 } },
    { name: 'bottom edge', at: { x: 100, y: 890 }, view: WIDE, want: { left: 100, top: 492 } },
    { name: 'bottom right', at: { x: 1399, y: 899 }, view: WIDE, want: { left: 1104, top: 492 } },
    { name: 'negative point', at: { x: -20, y: -5 }, view: WIDE, want: { left: 8, top: 8 } },
    {
      name: 'phone',
      at: { x: 300, y: 700 },
      view: { width: 390, height: 800 },
      want: { left: 94, top: 392 },
    },
    {
      name: 'taller than the window',
      at: { x: 50, y: 300 },
      view: { width: 390, height: 300 },
      want: { left: 50, top: 8 },
    },
  ])('$name', ({ at, view, want }) => {
    expect(clampToView(at, SIZE, view)).toEqual(want)
  })

  it.each([
    [0, 0],
    [1399, 899],
    [700, 450],
    [1, 898],
  ])('keeps a popup opened at (%i, %i) inside the window', (x, y) => {
    const { left, top } = clampToView({ x, y }, SIZE, WIDE)
    expect(left).toBeGreaterThanOrEqual(8)
    expect(top).toBeGreaterThanOrEqual(8)
    expect(left + SIZE.width).toBeLessThanOrEqual(WIDE.width - 8)
    expect(top + SIZE.height).toBeLessThanOrEqual(WIDE.height - 8)
  })
})
