import { useLayoutEffect } from 'react'
import type { RefObject } from 'react'

/** The custom property of the table's main area that holds how far the prompt sheet stands above the shared strip, in px. */
export const SHEET_OVERHANG_VAR = '--sheet-overhang'

/**
 * Keeps `SHEET_OVERHANG_VAR` on the table's main area equal to how far the sheet grows above the strip it stands in
 * (its height less the strip's), so that the opponents' area leaves that room free and the sheet lies over no card.
 * It follows the sheet with a `ResizeObserver`, sets a property and not React state, so a prompt that grows or shrinks
 * the sheet redraws nothing but the opponents' padding.
 */
export function useSheetOverhang(
  strip: RefObject<HTMLElement | null>,
  sheet: RefObject<HTMLElement | null>,
): void {
  useLayoutEffect(() => {
    const row = strip.current
    const lid = sheet.current
    const main = row?.closest<HTMLElement>('[data-slot="table-main"]')
    if (row == null || lid == null || main == null) return undefined
    const publish = (): void => {
      main.style.setProperty(SHEET_OVERHANG_VAR, `${Math.max(0, lid.offsetHeight - row.offsetHeight)}px`)
    }
    publish()
    const observer = typeof ResizeObserver === 'function' ? new ResizeObserver(publish) : null
    observer?.observe(lid)
    observer?.observe(row)
    return () => {
      observer?.disconnect()
      main.style.removeProperty(SHEET_OVERHANG_VAR)
    }
  }, [strip, sheet])
}
