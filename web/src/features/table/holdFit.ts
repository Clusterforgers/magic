/**
 * Pure hysteresis of a board's card size: a fit holds its card width and shape while they still fit, steps down only
 * when they no longer do and up only when clearly more room is there, so one permanent more or less moves nothing.
 */
import type { RowKind } from './cardModel'
import { fitAt, fitLanes, SHAPES } from './laneFit'
import type { BandPlace, BoardShape, LaneCounts, LaneFit, LaneLimits, LanePlace } from './laneFit'
import type { FitBox } from './fit'

/** The card width and shape a board drew last, which its next fit holds where it can. */
export interface FitHold {
  width: number
  shape: BoardShape
}

/** A held card grows only when the room fits a card this share wider: no flip-flop on one permanent. */
export const GROW_SHARE = 0.15

/** The share each rung of the card ladder is under the one above it: sizes a board steps between. */
export const RUNG_SHARE = 0.92

/** The card widths a held board steps between, from 224 px down to the 44 px floor, each about 8 % under the last. */
export const CARD_RUNGS: readonly number[] = (() => {
  const rungs: number[] = []
  for (let width = 224; width > 44; width = Math.round((width * RUNG_SHARE) / 2) * 2) rungs.push(width)
  rungs.push(44)
  return rungs
})()

/** The largest rung at or under `width`, never under `floor` nor over `width`. */
export function rungAtMost(width: number, floor: number): number {
  const rung = CARD_RUNGS.find((each) => each <= width) ?? floor
  return Math.min(width, Math.max(floor, rung))
}

/**
 * The fit of `counts` in `box` that holds `hold`: the held width and shape while they fit and no rung `GROW_SHARE`
 * wider does; otherwise the largest rung that fits. A zone that scrolls at the floor answers as `fitLanes` does.
 */
export function fitHeld(
  box: FitBox,
  counts: LaneCounts,
  facing: 'near' | 'far',
  limits: LaneLimits,
  hold: FitHold | null,
): LaneFit {
  const fresh = fitLanes(box, counts, facing, limits)
  if (fresh.scrolls) return fresh
  const floor = Math.round(limits.minWidth)
  const rung = rungAtMost(fresh.cardWidth, floor)
  const stepped = (): LaneFit => fitAt(box, counts, facing, limits, rung, hold?.shape) ?? fresh
  if (hold === null || hold.width > fresh.cardWidth) return stepped()
  // Clear room: the larger card also holds one permanent more in every lane, so the next one does not step it back.
  if (fresh.cardWidth >= hold.width * (1 + GROW_SHARE)) {
    const roomy = fitLanes(box, padded(counts), facing, limits)
    const grown = rungAtMost(roomy.cardWidth, floor)
    if (!roomy.scrolls && grown > hold.width) {
      return fitAt(box, counts, facing, limits, grown, hold.shape) ?? stepped()
    }
  }
  return fitAt(box, counts, facing, limits, hold.width, hold.shape) ?? stepped()
}

/** `counts` with one box more in every lane that has one. */
export function padded(counts: LaneCounts): LaneCounts {
  const more = (count: number): number => (count > 0 ? count + 1 : 0)
  return {
    ...counts,
    lands: more(counts.lands),
    creatures: more(counts.creatures),
    other: more(counts.other),
  }
}

/** The heights a board's bands keep while its card width and shape hold: by band (`bandKey`), in px. */
export interface BandHold {
  width: number
  shape: BoardShape
  heights: Readonly<Record<string, number>>
}

/** The key of a band: its lanes, in order, joined by `-`. */
export const bandKey = (lanes: readonly RowKind[]): string => lanes.join('-')

/** The key of the band of `shape` that a placed band belongs to, whichever of its lanes are empty. */
function keyIn(shape: BoardShape, band: BandPlace): string {
  const lane = band.lanes[0]?.lane
  return bandKey(SHAPES[shape].find((lanes) => lane !== undefined && lanes.includes(lane)) ?? [])
}

/**
 * The heights the bands of `fit` draw at: each at least the height it was drawn at since the last size step, so a line
 * that empties or a pile that shortens moves no lane beyond it, and a band that emptied keeps its room. A new card
 * width or shape, a scrolling fit or held heights that no longer fit `boxHeight` start afresh from the fit.
 */
export function holdBands(
  hold: BandHold | null,
  fit: LaneFit | null,
  boxHeight: number,
  bandGap: number,
): BandHold | null {
  if (fit === null || fit.scrolls) return null
  const fresh: Record<string, number> = {}
  for (const band of fit.bands) fresh[keyIn(fit.shape, band)] = band.height
  const start: BandHold = { width: fit.cardWidth, shape: fit.shape, heights: fresh }
  if (hold === null || hold.width !== fit.cardWidth || hold.shape !== fit.shape) return start
  const heights: Record<string, number> = { ...hold.heights }
  for (const [key, height] of Object.entries(fresh)) heights[key] = Math.max(heights[key] ?? 0, height)
  const values = Object.values(heights)
  const total = values.reduce((sum, height) => sum + height, 0) + bandGap * Math.max(0, values.length - 1)
  return total <= boxHeight ? { ...start, heights } : start
}

/** A band as drawn: its key, its lanes (none for a band held empty) and the height it keeps in px. */
export interface HeldBand {
  key: string
  lanes: readonly LanePlace[]
  height: number
  /** The px kept free at its right (`BandPlace.inset`). */
  inset?: number
}

/**
 * The bands to draw from the top of the zone down: the fit's bands, and an empty band in its shape's place for each
 * band held that the fit no longer has, each at the height `hold` keeps.
 */
export function heldBands(fit: LaneFit, hold: BandHold | null, facing: 'near' | 'far'): readonly HeldBand[] {
  const drawn = fit.bands.map((band) => ({ key: keyIn(fit.shape, band), band }))
  if (hold === null)
    return drawn.map(({ key, band }) => ({ key, lanes: band.lanes, height: band.height, inset: band.inset }))
  const placed = new Map(drawn.map(({ key, band }) => [key, band]))
  const outwards = SHAPES[fit.shape].map(bandKey)
  const keys = outwards.filter((key) => placed.has(key) || (hold.heights[key] ?? 0) > 0)
  // The fit's own order says which way its bands run (a scrolling opponent's board leads with its creatures).
  const first = outwards.find((key) => placed.has(key))
  const inwards = drawn.length > 1 ? drawn[0]?.key !== first : facing === 'far'
  const ordered = inwards ? [...keys].reverse() : keys
  return ordered.map((key) => {
    const band = placed.get(key)
    const height = Math.max(band?.height ?? 0, hold.heights[key] ?? 0)
    return { key, lanes: band?.lanes ?? [], height, inset: band?.inset }
  })
}
