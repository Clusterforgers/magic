/** What the popups of a card share (the manual menu and the list of clauses): focus back, a press outside, a place. */
import { useEffect, useLayoutEffect } from 'react'
import type { RefObject } from 'react'
import type { MenuStore, MenuTarget } from './menuStore'

const MARGIN = 8

/** Where a popup is drawn: inside the dialog its card sits in, else on the page. */
export function containerOf(target: MenuTarget): Element {
  return target.opener?.closest('[role="dialog"]') ?? document.body
}

/** Gives the focus back to the card that asked when the popup goes. */
export function useReturnFocus(target: MenuTarget): void {
  useEffect(() => {
    const { opener } = target
    return () => {
      if (opener?.isConnected === true) opener.focus({ preventScroll: true })
    }
  }, [target])
}

/** Closes the popup on a press outside of it. */
export function useCloseOutside(panel: RefObject<HTMLElement | null>, store: MenuStore | null): void {
  useEffect(() => {
    const away = (event: PointerEvent): void => {
      if (event.target instanceof Node && panel.current?.contains(event.target) !== true) store?.close()
    }
    document.addEventListener('pointerdown', away, true)
    return () => document.removeEventListener('pointerdown', away, true)
  }, [panel, store])
}

/** Where a popup of `size` goes so that it lies inside a window of `view`, `margin` from every edge. */
export function clampToView(
  at: { x: number; y: number },
  size: { width: number; height: number },
  view: { width: number; height: number },
  margin = MARGIN,
): { left: number; top: number } {
  const fit = (point: number, extent: number, room: number): number =>
    Math.max(margin, Math.min(point, room - extent - margin))
  return { left: fit(at.x, size.width, view.width), top: fit(at.y, size.height, view.height) }
}

/**
 * Places the popup at the point asked for, then moves it by what the browser shows so that it lies inside the window.
 * It places again when the popup changes size (its content grows) and when the window is resized.
 */
export function usePlaced(panel: RefObject<HTMLElement | null>, target: MenuTarget, again?: unknown): void {
  useLayoutEffect(() => {
    const element = panel.current
    if (element === null) return
    const place = (left: number, top: number): void => {
      element.style.setProperty('--mx', `${left}px`)
      element.style.setProperty('--my', `${top}px`)
    }
    const fit = (): void => {
      place(target.x, target.y)
      const box = element.getBoundingClientRect()
      if (box.width === 0) return
      const { left, top } = clampToView(target, box, { width: window.innerWidth, height: window.innerHeight })
      // A container that moves its fixed children shifts them by its own corner.
      place(left + (target.x - box.left), top + (target.y - box.top))
    }
    fit()
    const watcher = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(fit)
    watcher?.observe(element)
    window.addEventListener('resize', fit)
    return () => {
      watcher?.disconnect()
      window.removeEventListener('resize', fit)
    }
  }, [panel, target, again])
}
