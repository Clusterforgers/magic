/** Pure fit of the hover picture: how wide it draws and where it stands beside the element that was hovered. */

/** The width the picture had before it was zoomed, in px: the least it draws at. */
export const HOVER_FLOOR_PX = 244
/** The widest the picture draws: twice the floor. */
export const HOVER_CEILING_PX = 488
/** The gap between the element and the picture, and between the picture and the window's edge, in px. */
export const HOVER_GAP_PX = 12
export const HOVER_EDGE_PX = 8

/** The box of the hovered element and the window, in viewport px. */
export interface HoverBox {
  left: number
  right: number
  top: number
}

/** Where the picture stands (`left`, `top`) and how wide it is. */
export interface HoverFit {
  left: number
  top: number
  width: number
}

/**
 * The picture beside the element, on the side with the room: the right one unless the left has more and the right
 * cannot hold the picture. It grows to twice its old width as far as the window's height and the room allow, never
 * below the old width, and stays inside the window in height.
 */
export function hoverFit(box: HoverBox, viewport: { width: number; height: number }): HoverFit {
  const right = viewport.width - (box.right + HOVER_GAP_PX) - HOVER_EDGE_PX
  const left = box.left - HOVER_GAP_PX - HOVER_EDGE_PX
  const tall = ((viewport.height - HOVER_EDGE_PX * 2) * 5) / 7
  const width = Math.floor(Math.max(HOVER_FLOOR_PX, Math.min(HOVER_CEILING_PX, tall, Math.max(right, left))))
  const height = (width * 7) / 5
  const onRight = right >= width || (left < width && right >= left)
  const at = onRight ? box.right + HOVER_GAP_PX : Math.max(HOVER_EDGE_PX, box.left - width - HOVER_GAP_PX)
  const top = Math.min(
    Math.max(HOVER_EDGE_PX, box.top - height / 3),
    Math.max(HOVER_EDGE_PX, viewport.height - height - HOVER_EDGE_PX),
  )
  return { left: at, top, width }
}
