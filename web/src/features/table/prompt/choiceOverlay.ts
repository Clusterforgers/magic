/**
 * The centre overlay a card prompt lays its cards out in: over both boards, with the shared strip under it keeping the
 * sentence and the buttons. A phone uses it for every choice of cards, so a prompt never scrolls inside its sheet, and
 * any window for a long list (more than `FILTER_FROM` cards), whose cards would otherwise scroll sideways in a thin row.
 */
import { createContext, useCallback, useContext, useLayoutEffect, useMemo, useState } from 'react'
import { FILTER_FROM } from './cardsText'

/** What a card prompt reads to lay its cards out in the overlay. */
export interface ChoiceOverlay {
  /** The element the cards are drawn into; null before it mounts. */
  host: HTMLElement | null
  /** The table is a phone's. */
  phone: boolean
  /** Opens the overlay while the caller holds it; the function returned lets it go. */
  claim: () => () => void
}

/** The table's overlay; none outside a table, where a prompt keeps its cards in place. */
export const ChoiceOverlayContext = createContext<ChoiceOverlay | null>(null)

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

/** The table's side of the overlay: the context value, whether it is open and the ref of its element. */
export function useChoiceOverlayHost(phone: boolean): {
  value: ChoiceOverlay
  open: boolean
  ref: (element: HTMLDivElement | null) => void
} {
  const [host, setHost] = useState<HTMLElement | null>(null)
  const [holders, setHolders] = useState(0)
  const claim = useCallback(() => {
    setHolders((count) => count + 1)
    return () => setHolders((count) => count - 1)
  }, [])
  const value = useMemo(() => ({ host, phone, claim }), [host, phone, claim])
  return { value, open: holders > 0, ref: setHost }
}
