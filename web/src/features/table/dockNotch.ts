/** The corner of your board that the controls dock's body lies over, or may, where it grows upwards past its cell. */
import { useLayoutEffect, useState } from 'react'
import type { RefObject } from 'react'
import type { FitBox, FitNotch } from './fit'

/** The room kept between your cards and the dock's edge, in px. */
export const NOTCH_GAP_PX = 4

/**
 * The height in px the dock's body takes at the common decisions (a choice of targets, a payment, an order): the corner
 * it grows over is kept clear at all times, so those decisions move no card; a taller one adds its own height.
 */
export const DOCK_DECISION_PX = 172

/** The dock's cell, which keeps the hand row's height. */
const DOCK_CELL = '[data-region="controls"]'

/** The dock's body, which grows upwards over the board on a short window (`ControlsDock`). */
const DOCK_BODY = '[data-region="controls"] [data-slot="dock-body"]'

/** A box's edges in window px. */
export interface Edges {
  left: number
  top: number
  right: number
  bottom: number
}

/** The part of `zone`'s content box under `body`, from the content's right and bottom edges; null where they do not meet. */
export function notchOf(
  zone: Edges,
  padding: { right: number; bottom: number },
  body: Edges,
): FitNotch | null {
  const right = zone.right - padding.right
  const bottom = zone.bottom - padding.bottom
  const width = Math.ceil(right - body.left + NOTCH_GAP_PX)
  const height = Math.ceil(bottom - body.top + NOTCH_GAP_PX)
  if (width <= NOTCH_GAP_PX || height <= NOTCH_GAP_PX || body.right <= zone.left) return null
  return { width, height }
}

const same = (a: FitNotch | null, b: FitNotch | null): boolean =>
  a === b || (a !== null && b !== null && a.width === b.width && a.height === b.height)

/**
 * The corner of `zone` the dock's body covers, followed as either changes size; null while they do not meet, or
 * when `on` is false (an opponent's board, a phone, whose dock is a bar under the hand).
 */
export function useDockNotch(
  zone: RefObject<HTMLElement | null>,
  on: boolean,
  box: FitBox | null,
): FitNotch | null {
  const [notch, setNotch] = useState<FitNotch | null>(null)
  useLayoutEffect(() => {
    const element = zone.current
    const body = on ? document.querySelector(DOCK_BODY) : null
    const cell = on ? document.querySelector(DOCK_CELL) : null
    if (element === null || body === null || cell === null) {
      setNotch(null)
      return
    }
    const measure = (): void => {
      const style = getComputedStyle(element)
      const px = (value: string): number => parseFloat(value) || 0
      const grown = body.getBoundingClientRect()
      const top = Math.min(grown.top, cell.getBoundingClientRect().bottom - DOCK_DECISION_PX)
      const next = notchOf(
        element.getBoundingClientRect(),
        { right: px(style.paddingRight), bottom: px(style.paddingBottom) },
        { left: grown.left, top, right: grown.right, bottom: grown.bottom },
      )
      setNotch((was) => (same(was, next) ? was : next))
    }
    measure()
    if (typeof ResizeObserver !== 'function') return
    const observer = new ResizeObserver(measure)
    observer.observe(body)
    observer.observe(cell)
    observer.observe(element)
    return () => observer.disconnect()
  }, [zone, on, box])
  return notch
}

/** `box` with the dock's corner as its notch. */
export function withNotch(box: FitBox | null, notch: FitNotch | null): FitBox | null {
  return box === null || notch === null ? box : { ...box, notch }
}
