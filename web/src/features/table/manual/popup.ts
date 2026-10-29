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

/** Places the popup at the point asked for, then moves it by what the browser shows so that it lies inside the window. */
export function usePlaced(panel: RefObject<HTMLElement | null>, target: MenuTarget, again?: unknown): void {
  useLayoutEffect(() => {
    const element = panel.current
    if (element === null) return
    const place = (left: number, top: number): void => {
      element.style.setProperty('--mx', `${left}px`)
      element.style.setProperty('--my', `${top}px`)
    }
    place(target.x, target.y)
    const box = element.getBoundingClientRect()
    if (box.width === 0) return
    const left = Math.max(MARGIN, Math.min(target.x, window.innerWidth - box.width - MARGIN))
    const top = Math.max(MARGIN, Math.min(target.y, window.innerHeight - box.height - MARGIN))
    // A container that moves its fixed children (a dialog) shifts them by its own corner.
    place(left + (target.x - box.left), top + (target.y - box.top))
  }, [panel, target, again])
}
