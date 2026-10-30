/**
 * Where a move starts and ends on screen (docs/ui-design.md, Playing is legible). The DOM is the only source: a card is
 * its `data-object-id` button, a zone is the `data-fx-anchor` of the hand dock and the shared strip's stack, or the zone
 * chip (`data-zone`) of the seat's pod (`data-seat`) or tile (`data-tile-seat`). Boxes are in viewport coordinates, the
 * layer the fliers live on.
 */
import type { ObjectId, PlayerId } from '@/protocol/generated'
import type { FxPlace } from './moves'

/** A box on screen, in px. */
export interface Box {
  readonly x: number
  readonly y: number
  readonly width: number
  readonly height: number
}

/** A place found on screen: its box, and whether it is the card itself (else a chip or an area that stands for it). */
export interface Spot {
  readonly box: Box
  readonly card: boolean
}

const FACE = '[data-slot="card-face"]'

/** The `data-fx-anchor` value of the stack. */
export const STACK_ANCHOR = 'stack'

/** How much of the stack's area, from its left edge, stands for the stack when no chip does: where the chips begin. */
export const STACK_REACH_PX = 160

/** The box of an element if some of it is on screen, else null. A tapped card is measured by its face, as seen. */
function boxOf(element: Element): Box | null {
  const face = element.hasAttribute('data-object-id') ? (element.querySelector(FACE) ?? element) : element
  const { left, top, right, bottom, width, height } = face.getBoundingClientRect()
  if (width <= 0 || height <= 0) return null
  if (right <= 0 || bottom <= 0 || left >= window.innerWidth || top >= window.innerHeight) return null
  return { x: left, y: top, width, height }
}

/** The first of the elements that `selectors` find, in order, that is on screen. */
function firstBox(root: ParentNode, selectors: readonly string[]): Box | null {
  for (const selector of selectors) {
    for (const element of root.querySelectorAll(selector)) {
      const box = boxOf(element)
      if (box !== null) return box
    }
  }
  return null
}

const seatAt = (seat: PlayerId): string[] => [`[data-seat="${seat}"]`, `[data-tile-seat="${seat}"]`]

/** Where to look for the zone of a seat, best first; each selector may find a chip, a picture or the whole pod. */
export function zoneSelectors(seat: PlayerId, zone: string): string[] {
  const chips = seatAt(seat).map((root) => `${root} [data-zone="${zone}"]`)
  const area = zone === 'battlefield' ? [`[data-seat-area="${seat}"]`] : []
  return [`[data-fx-anchor="${seat}:${zone}"]`, ...chips, ...area, ...seatAt(seat)]
}

/** The shared strip's stack (the area the prompt and the chips stand in) if some of it is on screen. */
export function measureStrip(root: ParentNode): Box | null {
  return firstBox(root, [`[data-fx-anchor="${STACK_ANCHOR}"]`])
}

/** The size of a card in `seat`'s hand if one is on screen. */
export function measureHandCard(root: ParentNode, seat: PlayerId): { width: number; height: number } | null {
  const box = firstBox(root, [`[data-fx-anchor="${seat}:hand"] [data-object-id]`])
  return box === null ? null : { width: box.width, height: box.height }
}

/** Where an object is on screen, as a card or a stack chip (both carry `data-object-id`), or null when it is not. */
export function measureObject(root: ParentNode, object: ObjectId): Spot | null {
  const box = firstBox(root, [`[data-object-id="${object}"]`])
  return box === null ? null : { box, card: true }
}

/**
 * Where a place is on screen: the card itself when the table draws it, else its zone's anchor, else null (the move is
 * left out). The stack is a chip, never a card.
 */
export function measurePlace(root: ParentNode, place: FxPlace): Spot | null {
  if ('stack' in place) {
    const anchor = `[data-fx-anchor="${STACK_ANCHOR}"]`
    const chip =
      place.object === undefined ? null : firstBox(root, [`${anchor} [data-object-id="${place.object}"]`])
    const area = chip === null ? firstBox(root, [anchor]) : null
    const box = chip ?? (area === null ? null : { ...area, width: Math.min(area.width, STACK_REACH_PX) })
    return box === null ? null : { box, card: false }
  }
  if (place.object !== undefined) {
    const box = firstBox(root, [`[data-object-id="${place.object}"]`])
    if (box !== null) return { box, card: true }
  }
  const box = firstBox(root, zoneSelectors(place.seat, place.zone))
  return box === null ? null : { box, card: false }
}
