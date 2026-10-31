/** Pure fit of the card preview: how large its picture is and where the panel stands, from the card it shows and the window. */
import { clampedHeight, HAND_CLAMP, HAND_PHONE_CLAMP } from './layout'
import { dockOf, SIDE_DOCK_FROM } from './preview'
import type { Anchor, Dock, Viewport } from './preview'

/** The picture's width before it was zoomed: beside a wide table's card, and on a narrow window. */
export const PICTURE_FLOOR = { side: 240, narrow: 160 } as const
/** The widest picture: twice the floor. */
export const PICTURE_CEILING = { side: 480, narrow: 320 } as const
/** The share of a wide window's width the panel may take, which keeps it to the side column and the margin of the boards. */
export const PANEL_SHARE = 0.3
/** The panel may always take this much of a wide window, in px, so a smaller window still gets a larger picture. */
export const PANEL_ALLOWED_MIN_PX = 384
/** The panel's padding on both sides, in px (`p-3`). */
export const PANEL_PADDING_PX = 24
/** What a side panel is wider than its picture, in px: the padding and the room of a scroll bar. */
export const PANEL_EXTRA_PX = 48
/** The least the panel's width on a wide window, in px (`w-72`). */
export const PANEL_MIN_PX = 288
/** The space between a top or bottom panel and the sides of the window, in px (`inset-x-2`). */
export const INSET_NARROW_PX = 8
/** The height the lines under the picture keep (name, cost, type line), in px, before they scroll. */
export const TEXT_ROOM_PX = 96
/** The gap between the panel and the card it shows or the hand dock, in px. */
export const GAP_PX = 8
/** The distance of a side panel from the top, below the top bar, in px. */
export const TOP_SIDE_PX = 64
/** The distance of a side panel from the side of the window, and from the bottom when it may reach the hand, in px. */
export const EDGE_PX = 12
/** The distance of a top panel from the top of the window, in px. */
export const TOP_NARROW_PX = 56
/** The distance of a bottom panel from the bottom of a narrow, short window, in px: it may cover the hand there. */
export const BOTTOM_OLD_PX = 64

/** A picture is 5 wide for every 7 high. */
const ASPECT = 7 / 5

/** Where the preview stands and how large it draws. */
export interface PreviewFit {
  dock: Dock
  /** The width of the card's picture in px; its height is 7/5 of it. */
  picture: number
  /** The panel's width in px for a side dock; null for a top or bottom one, which spans the window. */
  width: number | null
  /** The tallest the panel grows in px; what is taller scrolls. */
  maxHeight: number
  /** The panel's distance from the top of the window (top and side docks), or null. */
  top: number | null
  /** The panel's distance from the bottom of the window (a bottom dock), or null. */
  bottom: number | null
  /** The text sits beside the picture (a window that is narrow and short) and not under it. */
  beside: boolean
}

/** The widest the picture goes for a height the panel may take: what the text lines leave of it. */
const pictureForHeight = (maxHeight: number): number => (maxHeight - PANEL_PADDING_PX - TEXT_ROOM_PX) / ASPECT

/** A wide window: the panel docks at the side away from its card, above the hand dock and the controls. */
function sideFit(anchor: Anchor, viewport: Viewport): PreviewFit {
  const dock = dockOf(anchor, viewport)
  const above = viewport.height - TOP_SIDE_PX - clampedHeight(HAND_CLAMP, viewport.height) - GAP_PX
  // A window too short to keep the hand dock clear and still show the old picture lets the panel reach it.
  const reach = viewport.height - TOP_SIDE_PX - EDGE_PX
  const maxHeight = above >= PICTURE_FLOOR.side * ASPECT + PANEL_PADDING_PX ? above : reach
  const clear =
    dock === 'right'
      ? viewport.width - EDGE_PX - (anchor.left + anchor.width + GAP_PX)
      : anchor.left - GAP_PX - EDGE_PX
  const fits = Math.min(
    PICTURE_CEILING.side,
    pictureForHeight(maxHeight),
    Math.max(viewport.width * PANEL_SHARE, PANEL_ALLOWED_MIN_PX) - PANEL_EXTRA_PX,
    clear - PANEL_EXTRA_PX,
  )
  const picture = Math.floor(Math.max(PICTURE_FLOOR.side, fits))
  return {
    dock,
    picture,
    width: Math.max(PANEL_MIN_PX, picture + PANEL_EXTRA_PX),
    maxHeight: Math.floor(maxHeight),
    top: TOP_SIDE_PX,
    bottom: null,
    beside: false,
  }
}

/** A tall narrow window: the panel docks above or below its card, whichever has the room, with the text under the picture. */
function portraitFit(anchor: Anchor, viewport: Viewport): PreviewFit {
  const bottomEdge = clampedHeight(HAND_PHONE_CLAMP, viewport.height) + GAP_PX
  const room: Record<'top' | 'bottom', number> = {
    top: anchor.top - GAP_PX - TOP_NARROW_PX,
    bottom: viewport.height - bottomEdge - (anchor.top + anchor.height + GAP_PX),
  }
  const need = PICTURE_FLOOR.narrow * ASPECT + PANEL_PADDING_PX + TEXT_ROOM_PX
  const first = dockOf(anchor, viewport) === 'top' ? 'top' : 'bottom'
  const other = first === 'top' ? 'bottom' : 'top'
  // Near the middle of the window the side the card's half points to may be the tight one.
  const dock = room[first] < need && room[other] > room[first] ? other : first
  const whole = viewport.height - TOP_NARROW_PX - bottomEdge
  const maxHeight = Math.min(whole, Math.max(room[dock], need))
  const fits = Math.min(
    PICTURE_CEILING.narrow,
    viewport.width - INSET_NARROW_PX * 2 - PANEL_PADDING_PX,
    pictureForHeight(maxHeight),
  )
  return {
    dock,
    picture: Math.floor(Math.max(PICTURE_FLOOR.narrow, fits)),
    width: null,
    maxHeight: Math.floor(maxHeight),
    top: dock === 'top' ? TOP_NARROW_PX : null,
    bottom: dock === 'bottom' ? Math.round(bottomEdge) : null,
    beside: false,
  }
}

/** A narrow and short window keeps the old size, with the text beside the picture. */
function shortFit(anchor: Anchor, viewport: Viewport): PreviewFit {
  const dock = dockOf(anchor, viewport)
  return {
    dock,
    picture: PICTURE_FLOOR.narrow,
    width: null,
    maxHeight: Math.max(0, viewport.height - TOP_NARROW_PX - BOTTOM_OLD_PX),
    top: dock === 'top' ? TOP_NARROW_PX : null,
    bottom: dock === 'bottom' ? BOTTOM_OLD_PX : null,
    beside: true,
  }
}

/**
 * Where the preview of the card in `anchor` stands in the window and how wide its picture is. A wide window docks it
 * at the side away from the card, a tall narrow one above or below the card; the picture grows to twice the old size
 * (240 px beside, 160 px narrow) as far as the window's height, its width and the room beside the card allow, and
 * never below the old size. The panel keeps clear of the card, and of the hand dock and the controls where the window
 * is tall enough to.
 */
export function previewFit(anchor: Anchor, viewport: Viewport): PreviewFit {
  if (viewport.width >= SIDE_DOCK_FROM) return sideFit(anchor, viewport)
  return viewport.height >= viewport.width ? portraitFit(anchor, viewport) : shortFit(anchor, viewport)
}
