/** Pure sizing of the hand dock: the card width its height allows, how far the cards overlap to fit its width, and how far a card rises. */
import { CARD_ASPECT, FIT_LIMITS } from './fit'

/** When the hand overlaps, at least this share of every card (but the last) still shows. */
export const HAND_MIN_SHOWN = 0.62

/** A hovered or focused card rises by this share of its height. */
export const HAND_LIFT = 0.4

/** Under a coarse pointer every card keeps this much of its width showing: a 44 px target. */
export const HAND_TOUCH_PX = 44

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
 * then an overlap just big enough, and past `HAND_MIN_SHOWN` of each card (or `strip` px, if that is more) a line
 * that scrolls.
 */
export function handFan(
  width: number,
  count: number,
  cardWidth: number,
  gap: number = FIT_LIMITS.gap,
  strip = 0,
): HandFan {
  if (count <= 1) return { gap, scrolls: false }
  const step = Math.floor((Math.max(0, width) - cardWidth) / (count - 1))
  if (step >= cardWidth + gap) return { gap, scrolls: false }
  const least = Math.min(cardWidth, Math.max(Math.ceil(cardWidth * HAND_MIN_SHOWN), strip))
  if (step >= least) return { gap: step - cardWidth, scrolls: false }
  return { gap: least - cardWidth, scrolls: true }
}

/** What `handLayout` needs. */
export interface HandInput {
  /** The room for the line in px, less the padding kept round it. */
  width: number
  /** The height the cards may take in px, less the padding. */
  height: number
  count: number
  /** A touch screen: every card keeps a strip of `HAND_TOUCH_PX` showing. */
  coarse?: boolean
}

/** The size, spacing and lift of the cards of a hand. */
export interface HandLayout {
  /** One card's box in px, 5:7. */
  box: { width: number; height: number }
  /** The space between neighbours in px, negative while they overlap. */
  gap: number
  /** The line is wider than the dock even at the most overlap, so it scrolls sideways. */
  scrolls: boolean
  /** How far a hovered or focused card rises, in px: `HAND_LIFT` of its height. */
  lift: number
}

/**
 * The cards as tall as the dock allows, side by side while they fit and overlapping by just enough before they do not.
 * A hand that would still scroll gets smaller cards first, down to the floor of the fit engine; a touch screen keeps
 * its card size and scrolls instead, since its strips are what a thumb needs.
 */
export function handLayout({ width, height, count, coarse = false }: HandInput): HandLayout {
  const strip = coarse ? HAND_TOUCH_PX : 0
  const widest = handCardWidth(height)
  let cardWidth = widest
  let fan = handFan(width, count, cardWidth, FIT_LIMITS.gap, strip)
  if (fan.scrolls && !coarse) {
    for (let next = widest - FIT_LIMITS.step; ; next -= FIT_LIMITS.step) {
      cardWidth = Math.max(next, FIT_LIMITS.minWidth)
      fan = handFan(width, count, cardWidth)
      if (!fan.scrolls || cardWidth === FIT_LIMITS.minWidth) break
    }
  }
  const boxHeight = Math.round(cardWidth * CARD_ASPECT)
  return {
    box: { width: cardWidth, height: boxHeight },
    gap: fan.gap,
    scrolls: fan.scrolls,
    lift: Math.round(boxHeight * HAND_LIFT),
  }
}
