/**
 * Card travel in the builder (docs/ui-design.md, Feel): a card added flies from where it was pressed into its line of
 * the deck list (or to the Deck switch while the deck pane is out of view) and the line beats as it lands; a line that
 * leaves the list slides out as a ghost while the list closes up at once. Motion only: the deck changes on the press,
 * nothing waits for a flight, and under reduced motion nothing moves. Plain DOM and the Web Animations API, no state.
 */
import { prefersReducedMotion } from '@/hooks/useReducedMotion'
import { arcBow } from '@/features/table/fx/travel'
import type { DeckZone } from '@/protocol/generated'

/** A card's flight into the list, and a line's slide out of it, in ms. */
export const TRAVEL_MS = 380
export const LEAVE_MS = 200

/** The element the flight lands on while the deck pane is out of view (the Deck switch on a narrow window). */
export const TRAVEL_TARGET = '[data-travel-target="deck"]'

interface Flight {
  key: string
  from: DOMRect
  name: string
}

// The one flight waiting for the render that draws its line; a newer add replaces it.
let waiting: Flight | null = null

const lineSelector = (key: string): string => `[data-line="${CSS.escape(key)}"]`
const keyOf = (zone: DeckZone, id: string): string => `${zone}:${id}`

function seen(element: Element | null): DOMRect | null {
  if (element === null) return null
  const box = element.getBoundingClientRect()
  return box.width > 0 && box.height > 0 ? box : null
}

const animates = (element: Element): boolean => typeof element.animate === 'function'

/**
 * Notes the card `id` going into `zone` of the deck, flying from `from` (the pressed row of the search, or the line it
 * leaves); `flyWaiting` sends it once the list has drawn its line.
 */
export function noteArrival(zone: DeckZone, id: string, name: string, from: Element | null): void {
  const box = prefersReducedMotion() ? null : seen(from)
  waiting = box === null ? null : { key: keyOf(zone, id), from: box, name }
}

/** The search's row of the card `id`, where a press on Add starts its flight. */
export function searchRowOf(id: string, root: ParentNode = document): Element | null {
  return root.querySelector(`li[data-card="${CSS.escape(id)}"]`)
}

/** The deck list's line of the card `id` in `zone`, if it is drawn. */
export function lineOf(zone: DeckZone, id: string, root: ParentNode = document): HTMLElement | null {
  return root.querySelector<HTMLElement>(lineSelector(keyOf(zone, id)))
}

/** Flies the waiting card to its line, or to the Deck switch when the line is out of view, and lets the line beat as it lands. */
export function flyWaiting(root: ParentNode = document): void {
  const flight = waiting
  waiting = null
  if (flight === null) return
  const line = root.querySelector<HTMLElement>(lineSelector(flight.key))
  const lineBox = seen(line)
  const target = lineBox ?? seen(root.querySelector(TRAVEL_TARGET))
  if (target === null) return
  const flier = document.createElement('div')
  flier.setAttribute('aria-hidden', 'true')
  flier.dataset['slot'] = 'deck-flier'
  flier.textContent = flight.name
  flier.className =
    'pointer-events-none fixed z-50 truncate rounded-lg border bg-raised px-2 py-1 text-sm font-medium text-foreground shadow-e2'
  const { from } = flight
  Object.assign(flier.style, {
    left: `${from.left}px`,
    top: `${from.top}px`,
    width: `${from.width}px`,
    height: `${Math.min(from.height, 44)}px`,
    transformOrigin: 'top left',
  })
  document.body.append(flier)
  if (!animates(flier)) {
    flier.remove()
    return
  }
  const dx = target.left - from.left
  const dy = target.top - from.top
  const bow = arcBow({ x: 0, y: 0 }, { x: dx, y: dy })
  const scale = Math.max(0.5, Math.min(1, target.width / from.width))
  const flight$ = flier.animate(
    [
      { transform: 'translate(0, 0) scale(1)', opacity: 1, easing: 'cubic-bezier(0.4, 0, 1, 1)' },
      { transform: 'translate(0, 0) scale(0.94)', opacity: 1, offset: 0.15 },
      {
        transform: `translate(${dx / 2 + bow.x}px, ${dy / 2 + bow.y}px) scale(${(1 + scale) / 2})`,
        opacity: 1,
        offset: 0.55,
        easing: 'cubic-bezier(0, 0, 0.2, 1)',
      },
      { transform: `translate(${dx}px, ${dy}px) scale(${scale})`, opacity: 0 },
    ],
    { duration: TRAVEL_MS, fill: 'forwards' },
  )
  void flight$.finished.then(
    () => flier.remove(),
    () => flier.remove(),
  )
  // The landing: the line (or the Deck switch) overshoots a little and settles as the card reaches it.
  const lands = lineBox === null ? root.querySelector(TRAVEL_TARGET) : line
  if (lands !== null && animates(lands))
    lands.animate(
      [
        { transform: 'scale(1)' },
        { transform: 'scale(1.03)', offset: 0.4 },
        { transform: 'scale(0.99)', offset: 0.75 },
        { transform: 'scale(1)' },
      ],
      { duration: 260, delay: TRAVEL_MS * 0.8, easing: 'ease-out' },
    )
}

/**
 * Lets the line of `id` in `zone` slide out of the list: a still copy of it takes its place over the page and fades
 * away to the right while the real line goes at once, so the list closes up in the same frame.
 */
export function noteLeaving(zone: DeckZone, id: string, root: ParentNode = document): void {
  if (prefersReducedMotion()) return
  const line = root.querySelector<HTMLElement>(lineSelector(keyOf(zone, id)))
  const box = seen(line)
  if (line === null || box === null) return
  const ghost = line.cloneNode(true) as HTMLElement
  ghost.removeAttribute('data-line')
  ghost.dataset['slot'] = 'deck-ghost'
  ghost.setAttribute('aria-hidden', 'true')
  ghost.inert = true
  Object.assign(ghost.style, {
    position: 'fixed',
    left: `${box.left}px`,
    top: `${box.top}px`,
    width: `${box.width}px`,
    height: `${box.height}px`,
    margin: '0',
    listStyle: 'none',
    pointerEvents: 'none',
    zIndex: '50',
  })
  document.body.append(ghost)
  if (!animates(ghost)) {
    ghost.remove()
    return
  }
  const leave = ghost.animate(
    [
      { transform: 'translateX(0) scale(1)', opacity: 1 },
      { transform: 'translateX(24px) scale(0.96)', opacity: 0 },
    ],
    { duration: LEAVE_MS, easing: 'cubic-bezier(0.3, 0, 1, 1)', fill: 'forwards' },
  )
  void leave.finished.then(
    () => ghost.remove(),
    () => ghost.remove(),
  )
}
