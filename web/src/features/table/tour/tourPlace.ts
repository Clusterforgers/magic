/** A box in window pixels. */
export interface Box {
  left: number
  top: number
  width: number
  height: number
}

/** Where the tour card goes relative to the part it explains. */
export type Side = 'below' | 'above' | 'right' | 'left' | 'over' | 'centre'

/** The card's place: its top-left corner and the side it chose. */
export interface Place {
  left: number
  top: number
  side: Side
}

/** The gap between the lit part and the card, and the margin the card keeps from the window's edges, in px. */
export const GAP = 12
export const MARGIN = 16

const clamp = (value: number, low: number, high: number): number =>
  Math.max(low, Math.min(value, Math.max(low, high)))

/**
 * Places a `card` of the given size next to the `target` within a `view`: below, above, right or left, the first side
 * with room; over the target's lower edge when none has room (a part as large as the window); in the centre without a
 * target. The card always stays inside the window's margins.
 */
export function placeCard(
  target: Box | null,
  card: { width: number; height: number },
  view: { width: number; height: number },
): Place {
  const maxLeft = view.width - MARGIN - card.width
  const maxTop = view.height - MARGIN - card.height
  if (target === null)
    return {
      left: clamp((view.width - card.width) / 2, MARGIN, maxLeft),
      top: clamp((view.height - card.height) / 2, MARGIN, maxTop),
      side: 'centre',
    }
  const bottom = target.top + target.height
  const right = target.left + target.width
  const across = clamp(target.left + (target.width - card.width) / 2, MARGIN, maxLeft)
  const down = clamp(target.top + (target.height - card.height) / 2, MARGIN, maxTop)
  if (bottom + GAP + card.height <= view.height - MARGIN)
    return { left: across, top: bottom + GAP, side: 'below' }
  if (target.top - GAP - card.height >= MARGIN)
    return { left: across, top: target.top - GAP - card.height, side: 'above' }
  if (right + GAP + card.width <= view.width - MARGIN) return { left: right + GAP, top: down, side: 'right' }
  if (target.left - GAP - card.width >= MARGIN)
    return { left: target.left - GAP - card.width, top: down, side: 'left' }
  return { left: across, top: clamp(bottom - card.height - GAP, MARGIN, maxTop), side: 'over' }
}

/** The lit box: the target grown by `pad` on each side and cut to the window. */
export function spotlight(target: Box, view: { width: number; height: number }, pad = 4): Box {
  const left = Math.max(0, target.left - pad)
  const top = Math.max(0, target.top - pad)
  const right = Math.min(view.width, target.left + target.width + pad)
  const bottom = Math.min(view.height, target.top + target.height + pad)
  return { left, top, width: Math.max(0, right - left), height: Math.max(0, bottom - top) }
}
