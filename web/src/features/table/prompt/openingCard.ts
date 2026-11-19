/** The card width of the floating opening hand (boss mode): one row in the overlay's room. */

/** The widest and narrowest card of the floating opening hand in px: seven of the narrowest fit a tablet's room in one row. */
export const OPENING_CARD_MOST_PX = 180
export const OPENING_CARD_LEAST_PX = 64
/** The share of the overlay's width and height the row may use, and the gap between cards in px. */
const OPENING_WIDTH_SHARE = 0.88
const OPENING_HEIGHT_SHARE = 0.5
const OPENING_GAP_PX = 8

/**
 * The width of a card of `count` in one row in an overlay of this room: the row takes most of the width and the cards
 * at most half the height and what `rest` (the panel's padding, sentence, answers and status in px) leaves them, between
 * `OPENING_CARD_LEAST_PX` and `OPENING_CARD_MOST_PX`.
 */
export function openingCardPx(room: { width: number; height: number }, count: number, rest = 0): number {
  const cards = Math.max(1, count)
  const across = Math.floor((room.width * OPENING_WIDTH_SHARE - (cards - 1) * OPENING_GAP_PX) / cards)
  const tall = Math.min(room.height * OPENING_HEIGHT_SHARE, room.height - rest)
  const down = Math.floor((tall * 5) / 7)
  return Math.max(OPENING_CARD_LEAST_PX, Math.min(OPENING_CARD_MOST_PX, across, down))
}
