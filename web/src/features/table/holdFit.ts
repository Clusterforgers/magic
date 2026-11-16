/**
 * Pure hysteresis of a board's card size: a fit holds its card width and shape while they still fit, steps down only
 * when they no longer do and up only when clearly more room is there, so one permanent more or less moves nothing.
 */
import { fitAt, fitLanes } from './laneFit'
import type { BoardShape, LaneCounts, LaneFit, LaneLimits } from './laneFit'
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
function padded(counts: LaneCounts): LaneCounts {
  const more = (count: number): number => (count > 0 ? count + 1 : 0)
  return {
    ...counts,
    lands: more(counts.lands),
    creatures: more(counts.creatures),
    other: more(counts.other),
  }
}
