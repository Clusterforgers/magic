/**
 * The centre overlay a card prompt lays its cards out in: a panel over both boards, with the shared strip under it
 * keeping the sentence and the buttons. A phone uses it for every choice of cards, so a prompt never scrolls inside its
 * sheet, and any window for a long list (more than `FILTER_FROM` cards), whose cards would otherwise scroll sideways in
 * a thin row. The panel is sized to its cards (`overlayLayout.ts`) from the room the table measures here.
 */
import { createContext, useCallback, useContext, useLayoutEffect, useMemo, useState } from 'react'
import type { Viewport } from '../layout'
import { FILTER_FROM } from './cardsText'
import type { OverlayRoom } from './overlayLayout'

/** What a card prompt reads to lay its cards out in the overlay. */
export interface ChoiceOverlay {
  /** The element the cards are drawn into; null before it mounts. */
  host: HTMLElement | null
  /** The table is a phone's. */
  phone: boolean
  /** The room inside the overlay's element: measured, or worked out from the window before it has a size. */
  room: OverlayRoom
  /** Opens the overlay while the caller holds it; the function returned lets it go. */
  claim: () => () => void
  /** The overlay is open. */
  open: boolean
  /** The opening-hand decision floats in the overlay (every table off a phone), with the seats' keep status in it. */
  opening: boolean
}

/** The table's overlay; none outside a table, where a prompt keeps its cards in place. */
export const ChoiceOverlayContext = createContext<ChoiceOverlay | null>(null)

/** The room before the element has a size (a test, a hidden box): most of the window's width, over half its height. */
export function guessRoom(viewport: Viewport, phone: boolean): OverlayRoom {
  return phone
    ? { width: viewport.width - 16, height: viewport.height * 0.5, phone }
    : { width: viewport.width * 0.78, height: viewport.height * 0.55, phone }
}

/** The room inside `box`, padding left out, in steps of 8 px so a pixel of change does not lay the cards out again. */
function roomOf(box: HTMLElement): { width: number; height: number } | null {
  const style = getComputedStyle(box)
  const across = (a: string, b: string): number => (parseFloat(a) || 0) + (parseFloat(b) || 0)
  const width = box.clientWidth - across(style.paddingLeft, style.paddingRight)
  const height = box.clientHeight - across(style.paddingTop, style.paddingBottom)
  if (!(width > 0 && height > 0)) return null
  return { width: Math.floor(width / 8) * 8, height: Math.floor(height / 8) * 8 }
}

/** Whether a choice among `count` cards takes the overlay: every choice on a phone, a long list anywhere. */
export function wantsOverlay(count: number, phone: boolean): boolean {
  return phone ? count > 0 : count > FILTER_FROM
}

/** The overlay's element for a choice among `count` cards, held open while the caller is mounted; null to draw in place. */
export function useChoiceOverlay(count: number): HTMLElement | null {
  const overlay = useContext(ChoiceOverlayContext)
  const host = overlay !== null && wantsOverlay(count, overlay.phone) ? overlay.host : null
  const claim = overlay?.claim
  const used = host !== null
  useLayoutEffect(() => (used && claim !== undefined ? claim() : undefined), [used, claim])
  return host
}

/** The overlay's element for the opening-hand decision where the table floats it (`ChoiceOverlay.opening`), held open while the caller is mounted; null to draw in the strip. */
export function useOpeningOverlay(): HTMLElement | null {
  const overlay = useContext(ChoiceOverlayContext)
  const host = overlay !== null && overlay.opening ? overlay.host : null
  const claim = overlay?.claim
  const used = host !== null
  useLayoutEffect(() => (used && claim !== undefined ? claim() : undefined), [used, claim])
  return host
}

/** Whether the opening-hand decision is open in the overlay, so the strip leaves out what the overlay carries. */
export function useOpeningInOverlay(): boolean {
  const overlay = useContext(ChoiceOverlayContext)
  return overlay !== null && overlay.opening && overlay.open
}

/** The room the open overlay has; a table's guess outside one. */
export function useOverlayRoom(): OverlayRoom {
  return useContext(ChoiceOverlayContext)?.room ?? { width: 960, height: 480, phone: false }
}

/** The table's side of the overlay: the context value, whether it is open and the ref of its element. */
export function useChoiceOverlayHost(
  phone: boolean,
  viewport: Viewport,
  opening = false,
): {
  value: ChoiceOverlay
  open: boolean
  ref: (element: HTMLDivElement | null) => void
} {
  const [host, setHost] = useState<HTMLElement | null>(null)
  const [holders, setHolders] = useState(0)
  const [measured, setMeasured] = useState<{ width: number; height: number } | null>(null)
  const claim = useCallback(() => {
    setHolders((count) => count + 1)
    return () => setHolders((count) => count - 1)
  }, [])
  const open = holders > 0
  // The element is hidden while closed and has no size then; it is followed while open.
  useLayoutEffect(() => {
    if (host === null || !open) return
    const measure = (): void =>
      setMeasured((before) => {
        const next = roomOf(host)
        return before?.width === next?.width && before?.height === next?.height ? before : next
      })
    measure()
    if (typeof ResizeObserver !== 'function') return
    const observer = new ResizeObserver(measure)
    observer.observe(host)
    return () => observer.disconnect()
  }, [host, open])
  const room = useMemo(
    () => (measured === null ? guessRoom(viewport, phone) : { ...measured, phone }),
    [measured, viewport, phone],
  )
  const value = useMemo(
    () => ({ host, phone, room, claim, open, opening: opening && !phone }),
    [host, phone, room, claim, open, opening],
  )
  return { value, open, ref: setHost }
}
