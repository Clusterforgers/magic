//! Goal: a board's bands keep their height until the next size step, so a line that empties, a pile that shortens or a lane that empties moves no lane beyond it on any board (yours from the top, an opponent's from the middle of the table), while a new card size, a new shape or a box too low for the held heights starts afresh.
import { describe, expect, it } from 'vitest'
import { fitHeld, heldBands, holdBands } from './holdFit'
import type { BandHold, FitHold } from './holdFit'
import { fitAt, LANE_LIMITS } from './laneFit'
import type { LaneCounts, LaneFit } from './laneFit'

const counts = (creatures: number, lands: number, other: number): LaneCounts => ({ creatures, lands, other })

/** The top of every band in px as the zone draws them: an opponent's board stands on the zone's foot. */
function tops(
  fit: LaneFit,
  hold: BandHold | null,
  facing: 'near' | 'far',
  boxHeight: number,
): Map<string, number> {
  const bands = heldBands(fit, hold, facing)
  const total =
    bands.reduce((sum, band) => sum + band.height, 0) + LANE_LIMITS.bandGap * Math.max(0, bands.length - 1)
  let top = facing === 'far' ? boxHeight - total : 0
  const out = new Map<string, number>()
  for (const band of bands) {
    out.set(band.key, top)
    top += band.height + LANE_LIMITS.bandGap
  }
  return out
}

/** Plays `steps` through a board as `Battlefield` does: the card size held, then the bands. */
function play(
  steps: readonly LaneCounts[],
  facing: 'near' | 'far',
  box = { width: 900, height: 420 },
): { fit: LaneFit; tops: Map<string, number> }[] {
  let size: FitHold | null = null
  let bands: BandHold | null = null
  return steps.map((step) => {
    const fit = fitHeld(box, step, facing, LANE_LIMITS, size)
    size = { width: fit.cardWidth, shape: fit.shape }
    bands = holdBands(bands, fit, box.height, LANE_LIMITS.bandGap)
    return { fit, tops: tops(fit, bands, facing, box.height) }
  })
}

describe('a board holds its bands until the next size step', () => {
  // Each case starts on a board whose lane runs onto a second line or a taller pile, then gives some back.
  it.each([
    ['far', 'a creature line that empties', [counts(14, 6, 3), counts(13, 6, 3), counts(9, 6, 3)]],
    ['far', 'a land pile that shortens', [counts(4, 12, 2), counts(4, 9, 2), counts(4, 5, 2)]],
    ['far', 'the other lane that empties', [counts(5, 6, 4), counts(5, 6, 0)]],
    ['near', 'a creature line that empties', [counts(14, 6, 3), counts(9, 6, 3)]],
  ] as const)('%s board: %s moves no band', (facing, _what, steps) => {
    const drawn = play(steps, facing)
    const first = drawn[0]
    if (first === undefined) throw new Error('no steps')
    const held = drawn
      .slice(1)
      .filter((now) => now.fit.cardWidth === first.fit.cardWidth && now.fit.shape === first.fit.shape)
    expect(held.length).toBeGreaterThan(0)
    for (const now of held) {
      for (const [key, top] of first.tops) expect(now.tops.get(key), key).toBe(top)
    }
  })

  it('keeps a band that emptied as room in its place', () => {
    const fit = fitHeld({ width: 900, height: 420 }, counts(6, 6, 3), 'far', LANE_LIMITS, null)
    const held = holdBands(null, fit, 420, LANE_LIMITS.bandGap)
    // The same card size, as when the room the creatures leave is not enough for a larger rung.
    const after = fitAt(
      { width: 900, height: 420 },
      counts(0, 6, 3),
      'far',
      LANE_LIMITS,
      fit.cardWidth,
      fit.shape,
    )
    if (after === null) throw new Error('no fit')
    const bands = heldBands(after, holdBands(held, after, 420, LANE_LIMITS.bandGap), 'far')
    expect(bands.map((band) => band.key)).toEqual(['lands-other', 'creatures'])
    expect(bands.map((band) => band.height)).toEqual(fit.bands.map((band) => band.height))
    expect(bands.find((band) => band.key === 'creatures')?.lanes).toEqual([])
  })

  it('starts afresh at a new card size or shape', () => {
    const fit = fitHeld({ width: 900, height: 420 }, counts(14, 6, 3), 'far', LANE_LIMITS, null)
    const held = holdBands(null, fit, 420, LANE_LIMITS.bandGap)
    const smaller: LaneFit = { ...fit, cardWidth: fit.cardWidth - 8, bands: fit.bands.slice(0, 1) }
    expect(holdBands(held, smaller, 420, LANE_LIMITS.bandGap)?.heights).toEqual(
      holdBands(null, smaller, 420, LANE_LIMITS.bandGap)?.heights,
    )
  })

  it('starts afresh when the held heights no longer fit the box', () => {
    const fit = fitHeld({ width: 900, height: 420 }, counts(14, 6, 3), 'far', LANE_LIMITS, null)
    const held = holdBands(null, fit, 420, LANE_LIMITS.bandGap)
    const fewer = fitHeld({ width: 900, height: 300 }, counts(4, 6, 3), 'far', LANE_LIMITS, {
      width: fit.cardWidth,
      shape: fit.shape,
    })
    const kept = holdBands(held, fewer, 300, LANE_LIMITS.bandGap)
    const total =
      Object.values(kept?.heights ?? {}).reduce((sum, height) => sum + height, 0) +
      LANE_LIMITS.bandGap * Math.max(0, Object.keys(kept?.heights ?? {}).length - 1)
    expect(total).toBeLessThanOrEqual(300)
  })

  it('holds nothing for a board that scrolls', () => {
    const fit = fitHeld({ width: 200, height: 60 }, counts(40, 30, 20), 'far', LANE_LIMITS, null)
    expect(fit.scrolls).toBe(true)
    expect(holdBands(null, fit, 60, LANE_LIMITS.bandGap)).toBeNull()
    expect(heldBands(fit, null, 'far').map((band) => band.height)).toEqual(
      fit.bands.map((band) => band.height),
    )
  })
})
