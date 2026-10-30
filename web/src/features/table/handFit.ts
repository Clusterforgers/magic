/** Pure sizing of the hand dock: the card width its height allows, the rows a large hand wraps into, and how far a card rises. */
import { CARD_ASPECT, FIT_LIMITS } from './fit'

/** A hovered or focused card rises by this share of its height. */
export const HAND_LIFT = 0.4

/** The narrowest card of a hand in px: its name (up to three lines of 9 letters) and its cost still read. */
export const HAND_FLOOR_PX = 68

/** The space between neighbours of a row in px. */
export const HAND_GAP_PX = 4

/** The most rows a hand wraps into. */
export const HAND_MAX_ROWS = 3

/**
 * The top of a card that always shows, in px: two lines of name and the cost row. A row that another row lies over
 * shows this much, so the name and the cost of every card read without a hover; it is also over the 44 px target of a
 * touch screen.
 */
export const HAND_HEAD_PX = 54

/** The widest card the hand dock of this height (px) holds in one line: its height over the card's shape, between the fit limits. */
export function handCardWidth(height: number): number {
  const fits = Number.isFinite(height) ? Math.floor(height / CARD_ASPECT) : FIT_LIMITS.minWidth
  return Math.max(FIT_LIMITS.minWidth, Math.min(FIT_LIMITS.maxWidth, fits))
}

/** What the hand dock draws round its cards in px: its top edge, its padding and the line's padding. */
export const HAND_DOCK_CHROME_PX = 17

/** What `handDockNeed` needs. */
export interface DockNeedInput {
  /** The room for the line in px, less the padding kept round it. */
  width: number
  /** The cards of the hand. */
  count: number
  /** The commanders that stand at the head of the dock, one card wide each and a divider after them. */
  commanders: number
  /** The tallest the dock may grow, in px. */
  most: number
}

/**
 * How tall the hand dock must be (px, with its chrome) to hold the hand in two or three rows of `HAND_FLOOR_PX` cards
 * without scrolling: 0 while one row of them fits the width, and 0 when the rows it needs would be taller than `most`
 * (the hand then scrolls in one row). It reads the width only, so a dock that grows to it does not change it.
 */
export function handDockNeed({ width, count, commanders, most }: DockNeedInput): number {
  const cardHeight = Math.round(HAND_FLOOR_PX * CARD_ASPECT)
  const head = commanders > 0 ? commanders * (HAND_FLOOR_PX + HAND_GAP_PX) + COMMAND_GAP_PX : 0
  const room = width - head
  const cards = Math.max(1, count)
  for (let rows = 1; rows <= HAND_MAX_ROWS && rows <= cards; rows += 1) {
    const perRow = Math.ceil(cards / rows)
    if (perRow * HAND_FLOOR_PX + (perRow - 1) * HAND_GAP_PX > room) continue
    if (rows === 1) return 0
    const need = cardHeight + (rows - 1) * HAND_HEAD_PX + HAND_DOCK_CHROME_PX
    return need <= most ? need : 0
  }
  return 0
}

/** The room between the commanders and the hand in the dock: the gap and the divider line. */
export const COMMAND_GAP_PX = 10

/** What the hand dock keeps free beside its cards in px, both sides: the padding of its section and of its line. */
export const HAND_DOCK_SIDES_PX = 16

/**
 * The width the hand dock needs in px to hold `count` cards in one row of `HAND_FLOOR_PX` cards, with `commanders` and
 * their divider at its head.
 */
export function handRowPx(count: number, commanders: number): number {
  const cards = Math.max(1, count)
  const head = commanders > 0 ? commanders * (HAND_FLOOR_PX + HAND_GAP_PX) + COMMAND_GAP_PX : 0
  return head + cards * HAND_FLOOR_PX + (cards - 1) * HAND_GAP_PX + HAND_DOCK_SIDES_PX
}

/** What `handLayout` needs. */
export interface HandInput {
  /** The room for the line in px, less the padding kept round it. */
  width: number
  /** The height the cards may take in px, less the padding. */
  height: number
  count: number
  /** A touch screen: a hand that does not fit keeps the full card size and scrolls, since a thumb needs the room. */
  coarse?: boolean
}

/** The size, rows and lift of the cards of a hand. */
export interface HandLayout {
  /** One card's box in px, 5:7. */
  box: { width: number; height: number }
  /** The space between neighbours of a row in px. */
  gap: number
  /** The rows the hand wraps into; the cards are in reading order, row by row. */
  rows: number
  /** The cards of the first (longest) row. */
  perRow: number
  /** How far a row lies over the row before it, in px: the card's height less `HAND_HEAD_PX`; 0 for one row. */
  overlap: number
  /** Even a single row of the smallest card is wider than the dock, so it scrolls sideways. */
  scrolls: boolean
  /** How far a hovered or focused card rises, in px: `HAND_LIFT` of its height. */
  lift: number
}

/**
 * The cards as tall as the dock allows in one row, none under another. A hand that does not fit shrinks the cards down
 * to `HAND_FLOOR_PX` first, then wraps into two or three balanced rows, each lying over the one before it so that its
 * top (name and cost) shows, and scrolls only when even that does not hold it.
 */
export function handLayout({ width, height, count, coarse = false }: HandInput): HandLayout {
  const widest = handCardWidth(height)
  const floor = Math.min(widest, HAND_FLOOR_PX)
  const room = Math.max(0, width)
  const cards = Math.max(1, count)
  const heightOf = (cardWidth: number) => Math.round(cardWidth * CARD_ASPECT)
  const make = (cardWidth: number, rows: number, perRow: number, scrolls: boolean): HandLayout => {
    const boxHeight = heightOf(cardWidth)
    return {
      box: { width: cardWidth, height: boxHeight },
      gap: HAND_GAP_PX,
      rows,
      perRow,
      overlap: rows > 1 ? Math.max(0, boxHeight - HAND_HEAD_PX) : 0,
      scrolls,
      lift: Math.round(boxHeight * HAND_LIFT),
    }
  }
  for (let rows = 1; rows <= HAND_MAX_ROWS && rows <= cards; rows += 1) {
    const perRow = Math.ceil(cards / rows)
    for (let cardWidth = widest; ; cardWidth = Math.max(floor, cardWidth - FIT_LIMITS.step)) {
      const boxHeight = heightOf(cardWidth)
      const stack = boxHeight + (rows - 1) * Math.min(boxHeight, HAND_HEAD_PX)
      const holds = rows === 1 || stack <= height
      if (holds && perRow * cardWidth + (perRow - 1) * HAND_GAP_PX <= room) {
        return make(cardWidth, rows, perRow, false)
      }
      if (cardWidth === floor) break
    }
  }
  return make(coarse ? widest : floor, 1, cards, true)
}
