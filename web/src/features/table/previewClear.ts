/** Pure placement of the card preview around what must stay readable and clickable: the open prompt's answers, the docks and the strip. */
import { clampedHeight, HAND_PHONE_CLAMP } from './layout'
import { SIDE_DOCK_FROM } from './preview'
import type { Anchor, Viewport } from './preview'
import {
  ASPECT,
  BOTTOM_OLD_PX,
  EDGE_PX,
  GAP_PX,
  INSET_NARROW_PX,
  PANEL_PADDING_PX,
  previewFit,
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

/**
 * What a kept box is: an answer button (`primary` for the main answer), the prompt's sentence, a seat's pod or tile
 * (`pod`: its life and figures, covered only after the card shown), or (none) a dock.
 */
export type ClearKind = 'primary' | 'answer' | 'text' | 'pod'

/** A box the preview keeps clear of; a `soft` one (the strip's spare room) it covers before any other. */
export interface ClearBox extends Anchor {
  soft?: boolean
  kind?: ClearKind
}

/** The edges of `anchor`. */
export const boxOf = (anchor: Anchor): Box => ({
  left: anchor.left,
  top: anchor.top,
  right: anchor.left + anchor.width,
  bottom: anchor.top + anchor.height,
})

/** Whether two boxes overlap. */
export const meets = (a: Box, b: Box): boolean =>
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

/** The least height a moved panel keeps: its whole picture and the panel's padding; the text under it scrolls. */
export const pictureHeight = (fit: PreviewFit): number =>
  Math.min(fit.maxHeight, Math.ceil(fit.picture * ASPECT + PANEL_PADDING_PX))

/** The rank of a kept box, most important first; a panel that covers fewer of the earlier ranks wins. */
const RANKS = ['primary', 'answer', 'dock', 'text', 'soft'] as const
type Rank = (typeof RANKS)[number] | 'pod'

const rankOf = (box: ClearBox): Rank => box.kind ?? (box.soft === true ? 'soft' : 'dock')

/** A kept box and its rank. */
interface Kept {
  box: Box
  rank: Rank
}

/**
 * The cost of a panel at `panel`, compared entry by entry: how many of the primary answer, the other answers, the docks,
 * the sentence and the strip it covers, then whether it covers the card it shows, how many pods, and whether any board card.
 */
function costOf(panel: Box, card: Box | null, kept: readonly Kept[], cards: readonly Box[]): number[] {
  const counts = RANKS.map(
    (rank) => kept.filter((each) => each.rank === rank && meets(panel, each.box)).length,
  )
  const shown = card !== null && meets(panel, card) ? 1 : 0
  const pods = kept.filter((each) => each.rank === 'pod' && meets(panel, each.box)).length
  return [...counts, shown, pods, cards.some((each) => meets(panel, each)) ? 1 : 0]
}

/** Whether cost `a` is lower than cost `b`. */
function cheaper(a: readonly number[], b: readonly number[]): boolean {
  for (let at = 0; at < a.length; at += 1) {
    const x = a[at] ?? 0
    const y = b[at] ?? 0
    if (x !== y) return x < y
  }
  return false
}

/** A vertical place of the panel: its top edge and its height, in window px. */
interface Spot {
  top: number
  height: number
}

/**
 * The places of a panel `tallest` px high at most and `least` at least, between `ceiling` and `floor`, that line up with
 * the edges of `edges` (each kept `GAP_PX` clear): at the ceiling, under a box, or ending above one.
 */
function spotsIn(
  ceiling: number,
  floor: number,
  tallest: number,
  least: number,
  edges: readonly Box[],
): Spot[] {
  const tops = new Set<number>([ceiling])
  for (const box of edges) {
    tops.add(Math.ceil(box.bottom + GAP_PX))
    tops.add(Math.floor(box.top - GAP_PX - least))
    tops.add(Math.floor(box.top - GAP_PX - tallest))
  }
  const spots: Spot[] = []
  for (const top of tops) {
    if (top < ceiling || top + least > floor) continue
    const most = Math.min(tallest, floor - top)
    const heights = new Set<number>([most, least])
    for (const box of edges) {
      const room = Math.floor(box.top - GAP_PX - top)
      if (room > least && room < most) heights.add(room)
    }
    for (const height of heights) spots.push({ top, height })
  }
  return spots
}

/** The candidate fits of a side panel at the full size of `base`: on either side, at every place the edges suggest. */
function sideCandidates(base: PreviewFit, viewport: Viewport, edges: readonly Box[]): PreviewFit[] {
  const own = base.dock === 'left' ? 'left' : 'right'
  const docks = [own, own === 'left' ? 'right' : 'left'] as const
  const floor = viewport.height - EDGE_PX
  const least = pictureHeight(base)
  return docks.flatMap((dock) => {
    const probe = panelBox({ ...base, dock }, viewport)
    const column = edges.filter((box) => box.left < probe.right && box.right > probe.left)
    return spotsIn(TOP_SIDE_PX, floor, base.maxHeight, least, column).map((spot) => ({
      ...base,
      dock,
      top: spot.top,
      bottom: null,
      maxHeight: spot.height,
    }))
  })
}

/**
 * The candidate fits of a top or bottom panel (a narrow window's) at the full size of `base`, inside the window's band:
 * a panel above the card grows up from its bottom edge, one below it down from its top edge.
 */
function bandCandidates(
  base: PreviewFit,
  anchor: Anchor,
  viewport: Viewport,
  edges: readonly Box[],
): PreviewFit[] {
  const portrait = viewport.height >= viewport.width
  const floor = portrait
    ? viewport.height - clampedHeight(HAND_PHONE_CLAMP, viewport.height) - GAP_PX
    : viewport.height - BOTTOM_OLD_PX
  const tallest = Math.max(base.maxHeight, floor - TOP_NARROW_PX)
  const middle = anchor.top + anchor.height / 2
  return spotsIn(TOP_NARROW_PX, floor, tallest, pictureHeight(base), edges).map((spot) => {
    const above = spot.top + spot.height / 2 < middle
    return {
      ...base,
      dock: above ? 'bottom' : 'top',
      top: above ? null : spot.top,
      bottom: above ? Math.round(viewport.height - spot.top - spot.height) : null,
      maxHeight: spot.height,
    }
  })
}

/**
 * Where the preview of the card in `anchor` stands, at the size `previewFit` gives it: the size never shrinks to stay
 * clear. Of the places on either side (a phone's: above or below the card) it takes the one that covers, in order, not
 * the primary answer, the fewest other answers, docks (`clear` without a kind), the sentence and the strip (`soft`), not
 * the card itself, the fewest pods and no board card of `cards` (a wide window's only); among equals the tallest, the natural place first.
 * A box without size is ignored.
 */
export function fitClear(
  anchor: Anchor,
  viewport: Viewport,
  clear: readonly ClearBox[],
  cards: readonly Anchor[] = [],
): PreviewFit {
  const base = previewFit(anchor, viewport)
  const side = viewport.width >= SIDE_DOCK_FROM
  const kept: Kept[] = clear
    .filter((box) => box.width > 0 && box.height > 0)
    .map((box) => ({ box: boxOf(box), rank: rankOf(box) }))
  const board = side ? cards.filter((card) => card.width > 0 && card.height > 0).map(boxOf) : []
  const card = anchor.width > 0 && anchor.height > 0 ? boxOf(anchor) : null
  const cost = (fit: PreviewFit): number[] => costOf(panelBox(fit, viewport), card, kept, board)
  let best = base
  let bestCost = cost(base)
  if (bestCost.every((each) => each === 0)) return base
  const edges = [...kept.map((each) => each.box), ...board, ...(card === null ? [] : [card])]
  const candidates = side
    ? sideCandidates(base, viewport, edges)
    : bandCandidates(base, anchor, viewport, edges)
  for (const fit of candidates) {
    const next = cost(fit)
    const tie = !cheaper(next, bestCost) && !cheaper(bestCost, next)
    if (cheaper(next, bestCost) || (tie && fit.maxHeight > best.maxHeight)) {
      best = fit
      bestCost = next
    }
  }
  return best
}
