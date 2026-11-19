import type { Box } from './resolve'

/** The room kept between the coach card and what it must not cover, and the window's edge. */
export const GAP = 12
export const EDGE = 8

/** What the placement needs to know. */
export interface PlaceInput {
  window: { width: number; height: number }
  /** The coach card's size. */
  card: { width: number; height: number }
  /** The lit boxes. */
  lit: readonly Box[]
  /** What the card must never cover: the band, the primary button, an open sheet, the preview. */
  clear: readonly Box[]
  /** The phone layout: the card is a bar across the window, docked at `top` or above `bottom`. */
  phone?: { top: number; bottom: number }
}

/** Where the card goes: its top left corner, and whether it had to overlap something (no free place). */
export interface Place {
  x: number
  y: number
  /** On a phone: the bar is folded to its title to find room. */
  folded: boolean
  /** No place was free of every lit and kept-clear box. */
  crowded: boolean
}

const overlap = (a: Box, b: Box, pad = 0): number => {
  const width = Math.min(a.x + a.width, b.x + b.width + pad) - Math.max(a.x, b.x - pad)
  const height = Math.min(a.y + a.height, b.y + b.height + pad) - Math.max(a.y, b.y - pad)
  return width > 0 && height > 0 ? width * height : 0
}

const clamp = (value: number, low: number, high: number): number =>
  Math.max(low, Math.min(value, Math.max(low, high)))

/** How much of `box` lies over the lit boxes (with the gap) and the kept-clear ones. */
function cost(box: Box, input: PlaceInput): number {
  const lit = input.lit.reduce((sum, each) => sum + overlap(box, each, GAP), 0)
  const clear = input.clear.reduce((sum, each) => sum + overlap(box, each), 0)
  return lit + clear * 4
}

/** The box of the whole lit area. */
function hull(boxes: readonly Box[]): Box | null {
  if (boxes.length === 0) return null
  const left = Math.min(...boxes.map((box) => box.x))
  const top = Math.min(...boxes.map((box) => box.y))
  const right = Math.max(...boxes.map((box) => box.x + box.width))
  const bottom = Math.max(...boxes.map((box) => box.y + box.height))
  return { x: left, y: top, width: right - left, height: bottom - top }
}

/** The first candidate free of every box, else the one that covers least. */
function best(candidates: readonly Box[], input: PlaceInput): { box: Box; crowded: boolean } | null {
  let chosen: { box: Box; cost: number } | null = null
  for (const box of candidates) {
    const each = cost(box, input)
    if (each === 0) return { box, crowded: false }
    if (chosen === null || each < chosen.cost) chosen = { box, cost: each }
  }
  return chosen === null ? null : { box: chosen.box, crowded: true }
}

/**
 * The coach card's place: beside the lit area on the first side with room (below, above, right, left), never over a
 * lit box or a kept-clear one; with no free side, the corner farthest from the lit area that is free, else the place
 * that covers least. On a phone the card is a bar docked under the top bar while the lit area is in the lower half and
 * above the hand otherwise, folded to its title when it has no room unfolded.
 */
export function coachPlace(input: PlaceInput): Place {
  const { window: win, card } = input
  const lit = hull(input.lit)
  const inside = (x: number, y: number, height = card.height): Box => ({
    x: clamp(x, EDGE, win.width - card.width - EDGE),
    y: clamp(y, EDGE, win.height - height - EDGE),
    width: card.width,
    height,
  })
  if (input.phone !== undefined) {
    const { top, bottom } = input.phone
    const low = lit === null || lit.y + lit.height / 2 > win.height / 2
    const docks = (height: number): Box[] => {
      const up = inside(EDGE, top + EDGE, height)
      const down = inside(EDGE, bottom - height - EDGE, height)
      // The docks first; then any height between them, for lit parts at both ends of the window.
      const between: Box[] = []
      for (let y = top + EDGE; y < bottom - height - EDGE; y += 24) between.push(inside(EDGE, y, height))
      return [...(low ? [up, down] : [down, up]), ...between]
    }
    const open = best(docks(card.height), input)
    if (open !== null && !open.crowded) return { x: open.box.x, y: open.box.y, folded: false, crowded: false }
    const folded = best(docks(Math.min(card.height, 96)), input)
    const box = folded?.box ?? inside(EDGE, top + EDGE)
    return { x: box.x, y: box.y, folded: true, crowded: folded?.crowded ?? true }
  }
  const centre =
    lit === null
      ? { x: win.width / 2, y: win.height / 2 }
      : { x: lit.x + lit.width / 2, y: lit.y + lit.height / 2 }
  const sides: Box[] =
    lit === null
      ? [inside(centre.x - card.width / 2, centre.y - card.height / 2)]
      : [
          inside(centre.x - card.width / 2, lit.y + lit.height + GAP),
          inside(centre.x - card.width / 2, lit.y - card.height - GAP),
          inside(lit.x + lit.width + GAP, centre.y - card.height / 2),
          inside(lit.x - card.width - GAP, centre.y - card.height / 2),
        ]
  const corners = [
    inside(EDGE, EDGE),
    inside(win.width, EDGE),
    inside(EDGE, win.height),
    inside(win.width, win.height),
  ].toSorted(
    (a, b) =>
      Math.hypot(b.x + b.width / 2 - centre.x, b.y + b.height / 2 - centre.y) -
      Math.hypot(a.x + a.width / 2 - centre.x, a.y + a.height / 2 - centre.y),
  )
  // Along the free edges between the corners too, so a crowded table still finds a strip of room.
  const steps = 6
  const edges: Box[] = []
  for (let at = 0; at <= steps; at += 1) {
    const x = ((win.width - card.width) * at) / steps
    const y = ((win.height - card.height) * at) / steps
    edges.push(inside(x, EDGE), inside(x, win.height), inside(EDGE, y), inside(win.width, y))
  }
  const chosen = best([...sides, ...corners, ...edges], input)
  const box = chosen?.box ?? inside(EDGE, EDGE)
  return { x: box.x, y: box.y, folded: false, crowded: chosen?.crowded ?? true }
}
