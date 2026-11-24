// Goal: the corner the controls dock lies over is measured from your board's content box, and every fit of lanes with that notch keeps each band that reaches into it beside the dock, across board sizes, counts and notches.
import { describe, expect, it } from 'vitest'
import { NOTCH_GAP_PX, notchOf, withNotch } from './dockNotch'
import { LANE_LIMITS, fitLanes } from './laneFit'
import type { LaneCounts } from './laneFit'

const zone = { left: 200, top: 300, right: 1200, bottom: 600 }
const padding = { right: 4, bottom: 4 }

describe('notchOf', () => {
  it.each([
    [
      { left: 800, top: 540, right: 1280, bottom: 720 },
      { width: 396 + NOTCH_GAP_PX, height: 56 + NOTCH_GAP_PX },
    ],
    [{ left: 900, top: 650, right: 1280, bottom: 720 }, null],
    [{ left: 1300, top: 400, right: 1400, bottom: 720 }, null],
  ])('the dock at %o covers %o', (body, notch) => {
    expect(notchOf(zone, padding, body)).toEqual(notch)
  })

  it('leaves a box without a notch as it is', () => {
    const box = { width: 900, height: 200 }
    expect(withNotch(box, null)).toBe(box)
    expect(withNotch(null, { width: 10, height: 10 })).toBeNull()
  })
})

const span = (lanes: readonly { span: number }[]): number =>
  lanes.reduce((sum, lane) => sum + lane.span, 0) + LANE_LIMITS.laneGap * (lanes.length - 1)

describe('fitLanes with a notch', () => {
  const COUNTS: LaneCounts[] = [
    { lands: 7, creatures: 4, other: 3 },
    { lands: 12, creatures: 9, other: 6 },
    { lands: 3, creatures: 14, other: 1 },
  ]
  const BOXES = [
    { width: 900, height: 170 },
    { width: 1100, height: 240 },
    { width: 700, height: 300 },
  ]
  const NOTCHES = [
    { width: 300, height: 60 },
    { width: 480, height: 90 },
  ]
  it('keeps every band that reaches into the notch beside it', () => {
    for (const counts of COUNTS)
      for (const box of BOXES)
        for (const notch of NOTCHES) {
          const fit = fitLanes({ ...box, notch }, counts, 'near')
          if (fit.scrolls) continue
          let top = 0
          for (const band of fit.bands) {
            if (top + band.height > box.height - notch.height) {
              expect(
                span(band.lanes),
                `${JSON.stringify(counts)} in ${box.width}x${box.height}`,
              ).toBeLessThanOrEqual(box.width - notch.width)
            }
            top += band.height + LANE_LIMITS.bandGap
          }
          // The notch only ever costs card size.
          expect(fit.cardWidth).toBeLessThanOrEqual(fitLanes(box, counts, 'near').cardWidth)
        }
  })
})
