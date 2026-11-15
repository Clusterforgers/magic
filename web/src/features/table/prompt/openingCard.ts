/** The card width of the floating opening hand (boss mode): one row in the overlay's room. */

/** The widest and narrowest card of the floating opening hand in px. */
export const OPENING_CARD_MOST_PX = 180
export const OPENING_CARD_LEAST_PX = 96
/** The share of the overlay's width and height the row may use, and the gap between cards in px. */
const OPENING_WIDTH_SHARE = 0.88
const OPENING_HEIGHT_SHARE = 0.5
const OPENING_GAP_PX = 8

/**
 * The width of a card of `count` in one row in an overlay of this room: the row takes most of the width and the cards
 * at most half the height, between `OPENING_CARD_LEAST_PX` and `OPENING_CARD_MOST_PX`.
 */
export function openingCardPx(room: { width: number; height: number }, count: number): number {
  const cards = Math.max(1, count)
  const across = Math.floor((room.width * OPENING_WIDTH_SHARE - (cards - 1) * OPENING_GAP_PX) / cards)
  const down = Math.floor((room.height * OPENING_HEIGHT_SHARE * 5) / 7)
  return Math.max(OPENING_CARD_LEAST_PX, Math.min(OPENING_CARD_MOST_PX, across, down))
}
