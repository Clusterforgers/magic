/** Pure fit of the card preview around what must stay readable and clickable: the open prompt's strip and the docks. */
import { clampedHeight, HAND_PHONE_CLAMP } from './layout'
import { SIDE_DOCK_FROM } from './preview'
import type { Anchor, Viewport } from './preview'
import {
  ASPECT,
  BOTTOM_OLD_PX,
  EDGE_PX,
  GAP_PX,
  INSET_NARROW_PX,
  PANEL_EXTRA_PX,
  PANEL_MIN_PX,
  PANEL_PADDING_PX,
  PICTURE_CEILING,
  PICTURE_FLOOR,
  pictureForHeight,
  previewFit,
  sideFit,
  TEXT_ROOM_PX,
  TOP_NARROW_PX,
  TOP_SIDE_PX,
} from './previewFit'
import type { PreviewFit } from './previewFit'

/** A rectangle by its four edges, in window px. */
export interface Box {
  left: number
  top: number
  right: number
  bottom: number
}

/** A stretch of the window's height, in px from the top. */
interface Stretch {
  top: number
  bottom: number
}

/** The least a picture is drawn when the panel is squeezed beside or above what it keeps clear of: the narrow window's. */
const SQUEEZED_PICTURE = PICTURE_FLOOR.narrow
/** The least height a squeezed panel keeps, in px: the squeezed picture and the panel's padding; the text scrolls. */
export const SQUEEZED_HEIGHT_PX = SQUEEZED_PICTURE * ASPECT + PANEL_PADDING_PX

const boxOf = (anchor: Anchor): Box => ({
  left: anchor.left,
  top: anchor.top,
  right: anchor.left + anchor.width,
  bottom: anchor.top + anchor.height,
})

const meets = (a: Box, b: Box): boolean =>
  a.left < b.right && a.right > b.left && a.top < b.bottom && a.bottom > b.top

/** The rectangle the panel of `fit` can take in the window, at its tallest. */
export function panelBox(fit: PreviewFit, viewport: Viewport): Box {
  if (fit.dock === 'left' || fit.dock === 'right') {
    const width = fit.width ?? 0
    const left = fit.dock === 'left' ? EDGE_PX : viewport.width - EDGE_PX - width
    const top = fit.top ?? TOP_SIDE_PX
    return { left, right: left + width, top, bottom: top + fit.maxHeight }
  }
  const top = fit.dock === 'top' ? (fit.top ?? 0) : viewport.height - (fit.bottom ?? 0) - fit.maxHeight
  return { left: INSET_NARROW_PX, right: viewport.width - INSET_NARROW_PX, top, bottom: top + fit.maxHeight }
}

/** A side panel at `dock` kept clear of `blockers`, beside them or above them, whichever draws the larger picture; null when neither fits. */
function sideClear(
  anchor: Anchor,
  viewport: Viewport,
  dock: 'left' | 'right',
  blockers: Box[],
): PreviewFit | null {
  const natural = sideFit(anchor, viewport, dock)
  const hit = blockers.filter((box) => meets(panelBox(natural, viewport), box))
  if (hit.length === 0) return natural
  const edge =
    dock === 'right'
      ? Math.max(...hit.map((box) => box.right)) + GAP_PX
      : Math.min(...hit.map((box) => box.left)) - GAP_PX
  const room = dock === 'right' ? viewport.width - EDGE_PX - edge : edge - EDGE_PX
  const beside = Math.floor(Math.min(natural.picture, room - PANEL_EXTRA_PX))
  const height = Math.floor(Math.min(...hit.map((box) => box.top)) - GAP_PX - TOP_SIDE_PX)
  const above = Math.floor(Math.min(natural.picture, Math.max(SQUEEZED_PICTURE, pictureForHeight(height))))
  const sideways = beside >= SQUEEZED_PICTURE
  const over = height >= SQUEEZED_HEIGHT_PX
  if (sideways && (!over || beside >= above)) {
    return { ...natural, picture: beside, width: beside + PANEL_EXTRA_PX }
  }
  if (over) {
    const width = Math.max(PANEL_MIN_PX, above + PANEL_EXTRA_PX)
    return { ...natural, picture: above, width, maxHeight: height }
  }
  return null
}

/** The side panel kept clear of `blockers` on the card's far side, else on the card's own side where that is clear of the card too. */
function sideAround(
  anchor: Anchor,
  viewport: Viewport,
  base: PreviewFit,
  blockers: Box[],
): PreviewFit | null {
  const own = base.dock === 'left' ? 'left' : 'right'
  const far = own === 'left' ? 'right' : 'left'
  const first = sideClear(anchor, viewport, own, blockers)
  const second = sideClear(anchor, viewport, far, [...blockers, boxOf(anchor)])
  if (first === null) return second
  return second !== null && second.picture > first.picture ? second : first
}

/** The free stretches of `band` between `blockers`, each kept `GAP_PX` clear of them, top first. */
function freeStretches(band: Stretch, blockers: Box[]): Stretch[] {
  const free: Stretch[] = []
  let cursor = band.top
  for (const box of [...blockers].sort((a, b) => a.top - b.top)) {
    if (box.top - GAP_PX > cursor) free.push({ top: cursor, bottom: Math.min(band.bottom, box.top - GAP_PX) })
    cursor = Math.max(cursor, box.bottom + GAP_PX)
  }
  if (band.bottom > cursor) free.push({ top: cursor, bottom: band.bottom })
  return free.filter((stretch) => stretch.bottom > stretch.top)
}

/** Where a full-width panel stands once it is kept clear: its dock, its edge and the height it may take. */
interface Placement {
  dock: 'top' | 'bottom'
  top: number | null
  bottom: number | null
  maxHeight: number
}

/**
 * A full-width panel (a top or bottom dock) placed in a free stretch of `band` between `blockers`: on the side of the
 * card it docked at when a stretch has `need` px there that the card does not take, else the other side, else the tallest
 * stretch; null when no stretch has `SQUEEZED_HEIGHT_PX`.
 */
function bandClear(
  base: PreviewFit,
  anchor: Anchor,
  viewport: Viewport,
  band: Stretch,
  blockers: Box[],
  need: number,
): Placement | null {
  const free = freeStretches(band, blockers)
  const card = boxOf(anchor)
  const place = (dock: 'top' | 'bottom', stretch: Stretch, height: number): Placement => ({
    dock,
    top: dock === 'top' ? stretch.top : null,
    bottom: dock === 'bottom' ? Math.round(viewport.height - stretch.bottom) : null,
    maxHeight: Math.floor(height),
  })
  // The part of a stretch the card leaves, on the side the panel docks at.
  const usable = (stretch: Stretch, dock: 'top' | 'bottom'): number => {
    if (card.bottom <= stretch.top || card.top >= stretch.bottom) return stretch.bottom - stretch.top
    return dock === 'top' ? card.top - GAP_PX - stretch.top : stretch.bottom - (card.bottom + GAP_PX)
  }
  const first = base.dock === 'bottom' ? 'bottom' : 'top'
  for (const dock of [first, first === 'top' ? 'bottom' : 'top'] as const) {
    const stretches = dock === 'top' ? free : [...free].reverse()
    const found = stretches.find((stretch) => usable(stretch, dock) >= need)
    if (found !== undefined) return place(dock, found, usable(found, dock))
  }
  const tallest = free.reduce<Stretch | null>(
    (best, stretch) =>
      best === null || stretch.bottom - stretch.top > best.bottom - best.top ? stretch : best,
    null,
  )
  if (tallest === null || tallest.bottom - tallest.top < SQUEEZED_HEIGHT_PX) return null
  return place(first, tallest, Math.min(need, tallest.bottom - tallest.top))
}

/** A tall narrow window: the panel above or below the card, inside the stretch the blockers leave. */
function portraitAround(
  base: PreviewFit,
  anchor: Anchor,
  viewport: Viewport,
  blockers: Box[],
): PreviewFit | null {
  const band = {
    top: TOP_NARROW_PX,
    bottom: viewport.height - clampedHeight(HAND_PHONE_CLAMP, viewport.height) - GAP_PX,
  }
  const need = PICTURE_FLOOR.narrow * ASPECT + PANEL_PADDING_PX + TEXT_ROOM_PX
  const placed = bandClear(base, anchor, viewport, band, blockers, need)
  if (placed === null) return null
  const fits = Math.min(
    PICTURE_CEILING.narrow,
    viewport.width - INSET_NARROW_PX * 2 - PANEL_PADDING_PX,
    pictureForHeight(placed.maxHeight),
  )
  return { ...base, ...placed, picture: Math.floor(Math.max(PICTURE_FLOOR.narrow, fits)) }
}

/** A narrow and short window: the old picture with the text beside it, inside the stretch the blockers leave. */
function shortAround(
  base: PreviewFit,
  anchor: Anchor,
  viewport: Viewport,
  blockers: Box[],
): PreviewFit | null {
  const band = { top: TOP_NARROW_PX, bottom: viewport.height - BOTTOM_OLD_PX }
  const placed = bandClear(base, anchor, viewport, band, blockers, SQUEEZED_HEIGHT_PX)
  return placed === null ? null : { ...base, ...placed }
}

/**
 * `previewFit` kept off every box of `clear` (the open prompt's strip, the hand dock and the controls, measured in
 * window px): a side panel moves to the other side, narrows beside the strip down to a 160 px picture or ends above it;
 * a top or bottom panel takes the free stretch of the window between them. A box without size is ignored; where
 * nothing is clear the panel stands where `previewFit` puts it.
 */
export function fitClear(anchor: Anchor, viewport: Viewport, clear: readonly Anchor[]): PreviewFit {
  const base = previewFit(anchor, viewport)
  const blockers = clear.filter((box) => box.width > 0 && box.height > 0).map(boxOf)
  const panel = panelBox(base, viewport)
  if (!blockers.some((box) => meets(panel, box))) return base
  if (viewport.width >= SIDE_DOCK_FROM) return sideAround(anchor, viewport, base, blockers) ?? base
  const moved =
    viewport.height >= viewport.width
      ? portraitAround(base, anchor, viewport, blockers)
      : shortAround(base, anchor, viewport, blockers)
  return moved ?? base
}
