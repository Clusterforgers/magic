import { useLayoutEffect, useState } from 'react'
import type { RefObject } from 'react'
import type { FitBox } from './fit'

function sameBox(a: FitBox | null, b: FitBox): boolean {
  return a !== null && a.width === b.width && a.height === b.height
}

/** The room inside a zone: its box less its padding, in whole px. Null where it has no width (a test, a hidden zone). */
function contentBox(zone: HTMLElement): FitBox | null {
  const style = getComputedStyle(zone)
  // An unset padding reads as an empty string, which is no padding.
  const px = (value: string): number => parseFloat(value) || 0
  const width = zone.clientWidth - px(style.paddingLeft) - px(style.paddingRight)
  const height = zone.clientHeight - px(style.paddingTop) - px(style.paddingBottom)
  // A zone squeezed to no height (a tall prompt sheet over it) still has a width its lanes must keep inside.
  if (!(width > 0)) return null
  return { width: Math.floor(width), height: Math.max(0, Math.floor(height)) }
}

/**
 * The room inside `ref`, followed with a `ResizeObserver`. Null until the element has a size, so a page without
 * layout (a test, a hidden zone) keeps the sizes it was given. The fit itself is `layoutSeat` (`rowStacks.ts`) for a battlefield.
 */
export function useContentBox(ref: RefObject<HTMLElement | null>): FitBox | null {
  const [box, setBox] = useState<FitBox | null>(null)
  useLayoutEffect(() => {
    const zone = ref.current
    if (zone === null) return
    const measure = (): void => {
      const next = contentBox(zone)
      if (next !== null) setBox((was) => (sameBox(was, next) ? was : next))
    }
    measure()
    if (typeof ResizeObserver !== 'function') return
    const observer = new ResizeObserver(measure)
    observer.observe(zone)
    return () => observer.disconnect()
  }, [ref])
  return box
}

/** The attribute that holds the transitions of a zone's cards off while its room is new (`feel.css`). */
export const STILL_ATTR = 'data-fit-still'

/**
 * Marks `ref` with `STILL_ATTR` from the commit in which `box` changes (the first measure included) until two frames
 * later, so cards laid out anew for their room stand in place at once: a load, a reload or a resize is a state, not a
 * move. Set in a layout effect, so the mark is on before the browser styles the new layout.
 */
export function useStillOnResize(ref: RefObject<HTMLElement | null>, box: FitBox | null): void {
  const width = box?.width
  const height = box?.height
  useLayoutEffect(() => {
    const element = ref.current
    if (element === null) return undefined
    element.setAttribute(STILL_ATTR, '')
    let frame = requestAnimationFrame(() => {
      frame = requestAnimationFrame(() => element.removeAttribute(STILL_ATTR))
    })
    return () => cancelAnimationFrame(frame)
  }, [ref, width, height])
}
