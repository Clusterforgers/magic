import { useEffect, useRef } from 'react'
import type { RefObject } from 'react'

/**
 * Scrolls a sideways strip back to its start when the item that leads it changes, so the lead is in view: the strip of
 * tiles leads with the seat whose turn it is, and a strip scrolled to a tile further along would hide it. A first draw
 * and a press on a tile change no lead, so they never move the strip.
 */
export function useLeadInView<Lead>(strip: RefObject<HTMLElement | null>, lead: Lead): void {
  const seen = useRef(lead)
  useEffect(() => {
    if (Object.is(seen.current, lead)) return
    seen.current = lead
    const element = strip.current
    if (element !== null && element.scrollLeft !== 0) element.scrollTo({ left: 0 })
  }, [strip, lead])
}
