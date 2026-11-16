/** FLIP moves of a list's items: an item a redraw puts elsewhere slides there from where it stood. */
import { useLayoutEffect, useRef } from 'react'
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
 * After every draw of `list`, slides each child named by `data-flip` from where it stood before the draw to where it
 * stands now (`flipOf`), as a transform on the Web Animations API: the layout itself changes at once, so nothing
 * waits. Under reduced motion the items stand at their new places at once.
 */
export function useFlip(list: RefObject<HTMLElement | null>): void {
  const reduced = useReducedMotion()
  const last = useRef(new Map<string, DOMRect>())
  useLayoutEffect(() => {
    const root = list.current
    if (root === null) return
    const now = new Map<string, DOMRect>()
    for (const item of root.children) {
      const key = item.getAttribute('data-flip')
      if (key !== null) now.set(key, item.getBoundingClientRect())
    }
    if (!reduced) {
      for (const item of root.children) {
        const key = item.getAttribute('data-flip')
        const from = key === null ? undefined : last.current.get(key)
        const to = key === null ? undefined : now.get(key)
        const frames = from === undefined || to === undefined ? null : flipOf(from, to)
        if (frames !== null && typeof item.animate === 'function') {
          item.animate(frames, { duration: FLIP_MS, easing: 'cubic-bezier(0.2, 0, 0, 1)' })
        }
      }
    }
    last.current = now
  })
}
