/** Pure sizes of a seat's corner: how large the pictures of its commanders, graveyard and exile are drawn in the room a pod column leaves. */
import { POD_COLUMN_PX } from './layout'

/** The space between the pictures of a corner in px. */
export const CORNER_GAP_PX = 4
/** The smallest picture a corner draws, the width of the smallest card the fit engine draws; under it the corner keeps its chips. */
export const CORNER_MIN_PX = 44
/** A picture is this share of the lane card width, so the corner reads smaller than the cards of the board beside it. */
export const CORNER_SHARE = 0.6
/** The padding of a pod panel (`p-2`) and the gap between its blocks (`gap-2`) in px. */
export const POD_PAD_PX = 8
export const POD_GAP_PX = 8
/** The width inside a pod panel in px: the pictures of a corner lie in it. */
export const CORNER_ROOM_PX = POD_COLUMN_PX - 2 * POD_PAD_PX
/** The px a second partner adds to its slot: its head shows above the one over it. */
export const PARTNER_HEAD_PX = 16

/** A card is this many times as high as wide. */
const ASPECT = 7 / 5

/** What `planCorner` needs. */
export interface CornerInput {
  /** The pictures to draw: the commanders' slot, the graveyard and the exile count one each. */
  slots: number
  /** The width inside the pod in px. */
  width: number
  /** The height left for the corner in px; null while the pod is not measured. */
  height: number | null
  /** The width of the creatures of the board beside the pod in px; null while it is not measured. */
  card: number | null
  /** The px the tallest slot grows by: a partner pair's second head. */
  extra?: number
}

/** The grid of pictures: their columns and their box in px. */
export interface CornerPlan {
  columns: number
  width: number
  height: number
}

const whole = (value: number): number => (Number.isFinite(value) ? Math.max(0, Math.floor(value)) : 0)

/** The height of a picture `width` px wide, in whole px. */
export function pictureHeight(width: number): number {
  return Math.round(whole(width) * ASPECT)
}

/** The width of a picture on a board of `card` px: `CORNER_SHARE` of it, at least `CORNER_MIN_PX`; the minimum while the board is not measured. */
export function pictureWidth(card: number | null): number {
  return card === null ? CORNER_MIN_PX : Math.max(CORNER_MIN_PX, whole(card * CORNER_SHARE))
}

/**
 * The largest picture of the grid that fits: at most `pictureWidth(card)`, in as many columns as give the widest
 * picture in the room (fewer rows win a tie). Null where even the least picture does not fit the height, so the corner
 * keeps its chips. With no height measured (a test, a hidden zone) the least picture is drawn in one row.
 */
export function planCorner(input: CornerInput): CornerPlan | null {
  const slots = whole(input.slots)
  if (slots === 0) return null
  const room = whole(input.width)
  const extra = whole(input.extra ?? 0)
  // A height that is not measured cannot be checked: the least picture, whatever the card.
  const cap = input.height === null ? CORNER_MIN_PX : pictureWidth(input.card)
  let best: CornerPlan | null = null
  for (let columns = slots; columns >= 1; columns -= 1) {
    const rows = Math.ceil(slots / columns)
    const across = Math.floor((room - (columns - 1) * CORNER_GAP_PX) / columns)
    const up =
      input.height === null
        ? Infinity
        : Math.floor((whole(input.height) - extra - (rows - 1) * CORNER_GAP_PX) / rows / ASPECT)
    const width = Math.min(cap, across, up)
    if (width >= CORNER_MIN_PX && (best === null || width > best.width)) {
      best = { columns, width, height: pictureHeight(width) }
    }
  }
  return best
}

/** The room a pod column leaves for the corner: its height less the pod's padding, the blocks above and the gap between. */
export function cornerHeight(column: number | null, above: number | null): number | null {
  if (column === null || above === null) return null
  return Math.max(0, column - 2 * POD_PAD_PX - above - POD_GAP_PX)
}
