import { useLayoutEffect, useMemo, useState } from 'react'
import type { RefObject } from 'react'
import { fitRows } from './fit'
import type { Fit, FitBox, FitGroup } from './fit'

/** A tapped card takes a square box one card high, which is this many cards wide. */
const TAPPED_WIDTH = 7 / 5

/** The stacks a row counts as in the fit: a tapped one needs the room of 1.4 cards, rounded up. */
export function stacksOf(count: number, tapped: number): number {
  return count + Math.ceil(Math.max(0, tapped) * (TAPPED_WIDTH - 1))
}

function sameBox(a: FitBox | null, b: FitBox): boolean {
  return a !== null && a.width === b.width && a.height === b.height
}

/** The room inside a zone: its box less its padding, in whole px. Null where nothing is laid out (a test, a hidden zone). */
function contentBox(zone: HTMLElement): FitBox | null {
  const style = getComputedStyle(zone)
  const width = zone.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight)
  const height = zone.clientHeight - parseFloat(style.paddingTop) - parseFloat(style.paddingBottom)
  if (!(width > 0) || !(height > 0)) return null
  return { width: Math.floor(width), height: Math.floor(height) }
}

/**
 * Fits the rows of a zone to the room it has. A `ResizeObserver` follows the zone's content box, and `fitRows` (pure,
 * `fit.ts`) answers with the largest card width at which every row fits, or the floor with the zone scrolling. Null
 * until the zone has a size, so a page without layout keeps the card sizes it was given. `groups` must be stable
 * between renders unless a row changed.
 */
export function useZoneFit(ref: RefObject<HTMLElement | null>, groups: readonly FitGroup[]): Fit | null {
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
  return useMemo(() => (box === null ? null : fitRows(box, groups)), [box, groups])
}
