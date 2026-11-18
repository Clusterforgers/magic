/** FLIP moves of a list's items: an item a redraw puts elsewhere slides there from where it stood. */
import { useLayoutEffect } from 'react'
import type { RefObject } from 'react'
import { useReducedMotion } from '@/hooks/useReducedMotion'

/** How long an item slides to its new place, in ms. */
export const FLIP_MS = 220
/** Moves of at most this many px are not animated: a sub-pixel settle reads as nothing. */
const STILL_PX = 1

/** The slide of an item from `from` to `to`, or null when it barely moved. */
export function flipOf(from: DOMRect, to: DOMRect): Keyframe[] | null {
  const dx = from.left - to.left
  const dy = from.top - to.top
  if (Math.abs(dx) <= STILL_PX && Math.abs(dy) <= STILL_PX) return null
  return [{ transform: `translate(${dx}px, ${dy}px)` }, { transform: 'none' }]
}

/**
 * Where each child of `root` named by `data-flip` stands inside the list's content (its scroll included), so a move of
 * the whole list (the page around it laid out afresh for a frame, a band that grows) is no move of its items.
 */
export function placesIn(root: HTMLElement): Map<string, DOMRect> {
  const origin = root.getBoundingClientRect()
  const places = new Map<string, DOMRect>()
  for (const item of root.children) {
    const key = item.getAttribute('data-flip')
    if (key === null) continue
    const box = item.getBoundingClientRect()
    const left = box.left - origin.left + root.scrollLeft
    places.set(key, new DOMRect(left, box.top - origin.top + root.scrollTop, box.width, box.height))
  }
  return places
}

/** The id of a slide this hook starts, so the next draw can stop it before it measures. */
const FLIP_ID = 'flip'

/**
 * On every draw of `list`, slides each child named by `data-flip` from where it stood on screen just before the draw to
 * where the draw puts it (`placesIn`, `flipOf`), as a transform on the Web Animations API: the layout itself changes
 * at once, so nothing waits. The place before is read while the draw renders, not kept from the draw before: the table
 * around a list may settle after the list's own effects ran, and a place kept from then would send a card running from
 * under the pointer at the next draw (a press redraws the card). A move of the list as a whole slides nothing. Under
 * reduced motion the items stand at their new places at once.
 */
export function useFlip(list: RefObject<HTMLElement | null>): void {
  const reduced = useReducedMotion()
  // Read during the render, before this draw changes the DOM: the places on screen now, a running slide included.
  const shown = list.current
  const before = shown === null || reduced ? null : placesIn(shown)
  useLayoutEffect(() => {
    const root = list.current
    if (root === null || before === null) return
    for (const item of root.children) {
      for (const running of item.getAnimations?.() ?? []) if (running.id === FLIP_ID) running.cancel()
    }
    const now = placesIn(root)
    for (const item of root.children) {
      const key = item.getAttribute('data-flip')
      const from = key === null ? undefined : before.get(key)
      const to = key === null ? undefined : now.get(key)
      const frames = from === undefined || to === undefined ? null : flipOf(from, to)
      if (frames !== null && typeof item.animate === 'function') {
        item.animate(frames, { id: FLIP_ID, duration: FLIP_MS, easing: 'cubic-bezier(0.2, 0, 0, 1)' })
      }
    }
  })
}
