import { useLayoutEffect, useState } from 'react'
import type { RefObject } from 'react'
import type { FitBox } from './fit'

function sameBox(a: FitBox | null, b: FitBox): boolean {
  return a !== null && a.width === b.width && a.height === b.height
}

/** The room inside a zone: its box less its padding, in whole px. Null where nothing is laid out (a test, a hidden zone). */
function contentBox(zone: HTMLElement): FitBox | null {
  const style = getComputedStyle(zone)
  // An unset padding reads as an empty string, which is no padding.
  const px = (value: string): number => parseFloat(value) || 0
  const width = zone.clientWidth - px(style.paddingLeft) - px(style.paddingRight)
  const height = zone.clientHeight - px(style.paddingTop) - px(style.paddingBottom)
  if (!(width > 0) || !(height > 0)) return null
  return { width: Math.floor(width), height: Math.floor(height) }
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
