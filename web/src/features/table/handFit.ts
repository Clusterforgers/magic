/** Pure sizing of the hand dock: the card width its height allows, and how far the cards overlap to fit its width. */
import { CARD_ASPECT, FIT_LIMITS } from './fit'

/** When the hand overlaps, at least this share of every card (but the last) still shows. */
export const HAND_MIN_SHOWN = 0.62

/** The widest card the hand dock of this height (px) holds in one line: its height over the card's shape, between the fit limits. */
export function handCardWidth(height: number): number {
  const fits = Number.isFinite(height) ? Math.floor(height / CARD_ASPECT) : FIT_LIMITS.minWidth
  return Math.max(FIT_LIMITS.minWidth, Math.min(FIT_LIMITS.maxWidth, fits))
}

/** How the cards of a hand sit in a line. */
export interface HandFan {
  /** The space between neighbours in px; negative when they overlap. */
  gap: number
  /** True when even the most overlap leaves the line wider than the dock, so it scrolls sideways. */
  scrolls: boolean
}

/**
 * The gap between cards that fits `count` cards of `cardWidth` in a dock `width` wide: the normal gap while they fit,
 * then an overlap just big enough, and past `HAND_MIN_SHOWN` of each card a line that scrolls.
 */
export function handFan(
  width: number,
  count: number,
  cardWidth: number,
  gap: number = FIT_LIMITS.gap,
): HandFan {
  if (count <= 1) return { gap, scrolls: false }
  const step = Math.floor((Math.max(0, width) - cardWidth) / (count - 1))
  if (step >= cardWidth + gap) return { gap, scrolls: false }
  const least = Math.ceil(cardWidth * HAND_MIN_SHOWN)
  if (step >= least) return { gap: step - cardWidth, scrolls: false }
  return { gap: least - cardWidth, scrolls: true }
}
