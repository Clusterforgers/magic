/** Pure fit of the hover preview outside the table: which side of the window it docks at and how wide its picture draws. */
import { dockOf } from '@/features/table/preview'

/** The least width the picture draws at, in px. */
export const HOVER_FLOOR_PX = 244
/** The widest the picture draws, in px: the table preview's ceiling. */
export const HOVER_CEILING_PX = 480
/** The gap between the hovered element and the panel, in px. */
export const HOVER_GAP_PX = 12
/** The distance of the panel from the window's side and bottom, in px. */
export const HOVER_EDGE_PX = 12
/** The distance of the panel from the top of the window, below the top bar, in px. */
export const HOVER_TOP_PX = 64
/** The panel's padding on both sides, in px (`p-3`). */
export const HOVER_PADDING_PX = 24
/** The height the lines under the picture keep (name, type line, support), in px. */
export const HOVER_TEXT_PX = 112

/** The box of the hovered element in viewport px. */
export interface HoverBox {
  left: number
  right: number
  top: number
  bottom?: number
}

/** Where the panel stands and how wide its picture draws. */
export interface HoverFit {
  /** The side of the window the panel docks at, away from the hovered element as the table's preview does. */
  dock: 'left' | 'right'
  /** The panel's distance from the left of the window, in px. */
  left: number
  /** The panel's distance from the top of the window, in px. */
  top: number
  /** The picture's width in px; the panel is `HOVER_PADDING_PX` wider. */
  width: number
  /** The tallest the panel grows, in px; what is taller scrolls out of sight. */
  maxHeight: number
}

/**
 * The preview docks at the side of the window away from the hovered element, below the top bar, as the table's card
 * preview does (`dockOf`). Its picture grows from the floor to the ceiling as far as the window's height (with the
 * lines under it) and the room between the element and that side allow, never below the floor, and never wider than
 * the window.
 */
export function hoverFit(box: HoverBox, viewport: { width: number; height: number }): HoverFit {
  const height = Math.max(0, (box.bottom ?? box.top) - box.top)
  const side = dockOf({ left: box.left, top: box.top, width: box.right - box.left, height }, viewport)
  const dock =
    side === 'left' || (side !== 'right' && box.left + (box.right - box.left) / 2 > viewport.width / 2)
      ? 'left'
      : 'right'
  const maxHeight = Math.max(0, viewport.height - HOVER_TOP_PX - HOVER_EDGE_PX)
  const tall = ((maxHeight - HOVER_PADDING_PX - HOVER_TEXT_PX) * 5) / 7
  const room =
    (dock === 'right' ? viewport.width - box.right : box.left) -
    HOVER_GAP_PX -
    HOVER_EDGE_PX -
    HOVER_PADDING_PX
  const across = viewport.width - HOVER_EDGE_PX * 2 - HOVER_PADDING_PX
  const width = Math.floor(Math.min(across, Math.max(HOVER_FLOOR_PX, Math.min(HOVER_CEILING_PX, tall, room))))
  const left = dock === 'right' ? viewport.width - HOVER_EDGE_PX - width - HOVER_PADDING_PX : HOVER_EDGE_PX
  return { dock, left: Math.max(HOVER_EDGE_PX, left), top: HOVER_TOP_PX, width, maxHeight }
}
