/** Scrolls the Commands panel so that a row that just opened is in view. */
import { useEffect } from 'react'
import type { RefObject } from 'react'

/**
 * Scrolls the panel that holds `row` (and only it, so the table never moves) by the least that brings the row's foot
 * into view, but never so far that its head leaves the top.
 */
export function revealRow(row: HTMLElement): void {
  const scroller = row.closest<HTMLElement>('[data-slot="commands-panel"]')
  if (scroller === null) return
  const box = row.getBoundingClientRect()
  const port = scroller.getBoundingClientRect()
  if (box.top < port.top) scroller.scrollTop -= port.top - box.top
  else if (box.bottom > port.bottom)
    scroller.scrollTop += Math.min(box.bottom - port.bottom, box.top - port.top)
}

/** Reveals the row of `ref` each time `open` turns true. */
export function useRevealOnOpen(ref: RefObject<HTMLElement | null>, open: boolean): void {
  useEffect(() => {
    if (open && ref.current !== null) revealRow(ref.current)
  }, [open, ref])
}
