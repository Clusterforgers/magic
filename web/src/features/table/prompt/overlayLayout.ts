/**
 * The panel of the long-list overlay: a compact plaque in the middle of the boards, as wide as its blocks of whole cards
 * (one block per seat that controls cards, led by its label) and no more than `PANEL_SHARE` of the room, scrolling
 * inside itself only when the cards cannot all stand in it.
 */
import type { ObjectId, ObjectView, PlayerId } from '@/protocol/generated'

/** The share of the room's width and height a panel takes at most on a desktop window; a phone's panel takes all of it. */
export const PANEL_SHARE = 0.8

/** The gap between cards and between the rows of a block, in px. */
export const CARD_GAP = 8

/** The gap between blocks side by side and between rows of blocks, in px. */
export const BLOCK_GAP = 16

/** The padding and border of the panel and the padding of its grid, both sides together, in px. */
export const PANEL_PAD = 40

/** What a scrollbar may take of the grid's width, in px, left free so the last block of a row never wraps for it. */
export const SCROLLBAR_PX = 16

/** A card is five wide to seven high. */
const CARD_RATIO = 7 / 5

/** What a seat's label over its block takes, in px, gap included. */
const LABEL_PX = 24

/** The card widths of a window, in px. */
interface CardSizes {
  /** The widest a card is drawn. */
  most: number
  /** The narrowest a card is drawn at where the room is narrow: the width grows from here to `most` with the room. */
  least: number
  /** The narrowest a card shrinks to so that all of them fit without scrolling. */
  floor: number
  /** The width kept when even the floor does not fit, so the cards stay readable and the panel scrolls. */
  kept: number
  /** The narrowest a scrolling panel draws a card, to show one whole row of them in a low window. */
  smallest: number
}

const DESKTOP: CardSizes = { most: 160, least: 120, floor: 104, kept: 120, smallest: 88 }
const PHONE: CardSizes = { most: 80, least: 80, floor: 64, kept: 76, smallest: 64 }

/** The room of the overlay in px, and whether it takes a phone's card sizes (a phone either way up). */
export interface OverlayRoom {
  width: number
  height: number
  phone: boolean
}

/** What `overlayLayout` answers: the card width, the columns of a block, the width of all blocks and whether the panel scrolls. */
export interface OverlayLayout {
  /** The width of a card in px. */
  card: number
  /** The columns of a block with at least that many cards. */
  columns: number
  /** The width the blocks take together, in px: the grid is as wide as that, so the browser wraps them as laid out. */
  width: number
  /** Not every card stands in the panel at once. */
  scrolls: boolean
}

/** The width a card is drawn at in a room: one tenth of a desktop room's width, between 120 and 160 px; a phone's is 80. */
export function cardWidthFor(room: OverlayRoom): number {
  const sizes = room.phone ? PHONE : DESKTOP
  return Math.round(Math.min(sizes.most, Math.max(sizes.least, room.width / 10)))
}

/** The width and height blocks of `sizes` cards take at `columns` columns and `card` px a card, wrapped in `inner` px. */
function packed(sizes: readonly number[], columns: number, card: number, inner: number) {
  const high = Math.round(card * CARD_RATIO)
  const label = sizes.length > 1 ? LABEL_PX : 0
  let width = 0
  let height = 0
  let rowWidth = 0
  let rowHeight = 0
  let inRow = 0
  for (const size of sizes) {
    const across = Math.min(columns, size)
    const blockWidth = across * card + (across - 1) * CARD_GAP
    const blockHeight = Math.ceil(size / across) * (high + CARD_GAP) - CARD_GAP + label
    if (inRow > 0 && rowWidth + BLOCK_GAP + blockWidth > inner) {
      width = Math.max(width, rowWidth)
      height += rowHeight + BLOCK_GAP
      rowWidth = 0
      rowHeight = 0
      inRow = 0
    }
    rowWidth += (inRow > 0 ? BLOCK_GAP : 0) + blockWidth
    rowHeight = Math.max(rowHeight, blockHeight)
    inRow += 1
  }
  return { width: Math.max(width, rowWidth), height: height + rowHeight }
}

/**
 * The layout for blocks of `sizes` cards in `room`, with `header` px of the panel taken by the search box and the
 * players' row. Cards are drawn at `cardWidthFor` the room, shrunk until all of them fit the panel's height down to a
 * floor, and kept readable (the panel scrolls) below it. At each width the columns are those that stack the blocks
 * lowest, the narrowest of equals.
 */
export function overlayLayout(room: OverlayRoom, sizes: readonly number[], header: number): OverlayLayout {
  const share = room.phone ? 1 : PANEL_SHARE
  const inner = Math.max(0, Math.floor(room.width * share) - PANEL_PAD - SCROLLBAR_PX)
  const available = Math.max(0, Math.floor(room.height * share) - PANEL_PAD - header)
  const bounds = room.phone ? PHONE : DESKTOP
  const biggest = sizes.reduce((a, b) => Math.max(a, b), 1)
  const layoutAt = (card: number): OverlayLayout => {
    const most = Math.max(1, Math.min(biggest, Math.floor((inner + CARD_GAP) / (card + CARD_GAP))))
    let best = { columns: 1, ...packed(sizes, 1, card, inner) }
    for (let columns = 2; columns <= most; columns += 1) {
      const fit = packed(sizes, columns, card, inner)
      if (fit.height < best.height || (fit.height === best.height && fit.width < best.width)) {
        best = { columns, ...fit }
      }
    }
    return { card, columns: best.columns, width: best.width, scrolls: best.height > available }
  }
  const wanted = cardWidthFor(room)
  for (let card = wanted; card >= bounds.floor; card -= 4) {
    const layout = layoutAt(card)
    if (!layout.scrolls) return layout
  }
  // Scrolling: cards stay readable, but a window too low for one whole row of them draws them smaller.
  const oneRow = Math.floor((available - (sizes.length > 1 ? LABEL_PX : 0)) / CARD_RATIO)
  return layoutAt(Math.max(bounds.smallest, Math.min(wanted, bounds.kept, oneRow)))
}

/** A run of cards one seat controls. */
export interface CardGroup {
  /** The seat that controls the cards; null when the cards are not grouped. */
  seat: PlayerId | null
  cards: ObjectId[]
}

/**
 * The cards in groups by the seat that controls them, seats in order and each group in the order it was given; one
 * group without a seat where `whole` (every card on offer) has one controller or none known, so a single player's list
 * carries no labels, and a search that narrows `cards` to one seat keeps that seat's label.
 */
export function groupByController(
  cards: readonly ObjectId[],
  whole: readonly ObjectId[],
  objects: ReadonlyMap<ObjectId, ObjectView | undefined>,
): CardGroup[] {
  const controllerOf = (id: ObjectId): PlayerId | undefined => objects.get(id)?.controller
  const seats = new Set(whole.map(controllerOf).filter((seat) => seat !== undefined))
  if (seats.size < 2) return [{ seat: null, cards: [...cards] }]
  const groups = new Map<PlayerId | null, ObjectId[]>()
  for (const id of cards) {
    const seat = controllerOf(id) ?? null
    groups.set(seat, [...(groups.get(seat) ?? []), id])
  }
  return [...groups]
    .sort(([a], [b]) => (a ?? Number.MAX_SAFE_INTEGER) - (b ?? Number.MAX_SAFE_INTEGER))
    .map(([seat, grouped]) => ({ seat, cards: grouped }))
}
