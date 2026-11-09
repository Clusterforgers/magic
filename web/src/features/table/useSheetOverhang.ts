import { useLayoutEffect } from 'react'
import type { RefObject } from 'react'

/** The custom property of the table's main area that holds how far the prompt sheet stands above the shared strip, in px. */
export const SHEET_OVERHANG_VAR = '--sheet-overhang'

/**
 * How far a sheet would stand above a strip `strip` px high with all of its content in view: its scroll height and
 * borders, so a sheet held at its most and scrolling inside itself still asks the split for room.
 */
export function wantedOverhang(
  sheet: Pick<HTMLElement, 'offsetHeight' | 'clientHeight' | 'scrollHeight'>,
  strip: number,
): number {
  const borders = Math.max(0, sheet.offsetHeight - sheet.clientHeight)
  return Math.max(0, sheet.offsetHeight - strip, sheet.scrollHeight + borders - strip)
}

/**
 * How far to scroll a box whose inside starts at `top` and is `height` px high so that `target` is in it: down until
 * its bottom shows, but never so far that its top leaves; 0 when it already shows.
 */
export function revealBy(top: number, height: number, target: { top: number; bottom: number }): number {
  const below = target.bottom - (top + height)
  if (!(below > 0)) return 0
  return Math.max(0, Math.min(below, target.top - top))
}

/**
 * Keeps `SHEET_OVERHANG_VAR` on the table's main area equal to how far the sheet grows above the strip it stands in
 * (its height less the strip's), so that the opponents' area leaves that room free and the sheet lies over no card.
 * It follows the sheet with a `ResizeObserver`, sets a property and not React state, so a prompt that grows or shrinks
 * the sheet redraws nothing but the opponents' padding. A sheet held at its most scrolls the prompt's answers into
 * view (`revealBy`) whenever it or its content resizes.
 */
export function useSheetOverhang(
  strip: RefObject<HTMLElement | null>,
  sheet: RefObject<HTMLElement | null>,
  /** Hears the overhang the sheet wants (`wantedOverhang`), which the board split keeps room for. */
  onOverhang?: (px: number) => void,
): void {
  useLayoutEffect(() => {
    const row = strip.current
    const lid = sheet.current
    const main = row?.closest<HTMLElement>('[data-slot="table-main"]')
    if (row == null || lid == null || main == null) return undefined
    const publish = (): void => {
      const overhang = Math.max(0, lid.offsetHeight - row.offsetHeight)
      main.style.setProperty(SHEET_OVERHANG_VAR, `${overhang}px`)
      onOverhang?.(wantedOverhang(lid, row.offsetHeight))
      const actions = [...lid.querySelectorAll('[data-slot="prompt-actions"]')].at(-1)
      if (actions === undefined) return
      const box = lid.getBoundingClientRect()
      lid.scrollTop += revealBy(box.top + lid.clientTop, lid.clientHeight, actions.getBoundingClientRect())
    }
    publish()
    const observer = typeof ResizeObserver === 'function' ? new ResizeObserver(publish) : null
    observer?.observe(lid)
    observer?.observe(row)
    // The content grows inside a sheet held at its most, which then keeps its size.
    if (lid.firstElementChild != null) observer?.observe(lid.firstElementChild)
    return () => {
      observer?.disconnect()
      main.style.removeProperty(SHEET_OVERHANG_VAR)
    }
  }, [strip, sheet, onOverhang])
}
