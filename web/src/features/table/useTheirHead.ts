/** Measures how far down the opponents' area their battlefields begin: what their pods and tile strip take. */
import { useLayoutEffect } from 'react'
import type { RefObject } from 'react'

/** The deepest of the opponents' battlefield zones below the top of their area, in px; 0 without a zone. */
export function headOf(areaTop: number, zoneTops: readonly number[]): number {
  return Math.max(0, ...zoneTops.map((top) => (Number.isFinite(top) ? top - areaTop : 0)))
}

/**
 * Tells `onHead` how far below the top of the opponents' area (inside `table`) their battlefields begin
 * (`headOf`), whenever the area or a board resizes: a pod line that wraps to several lines in a narrow column takes
 * more than the split's estimate, and the split and the prompt sheet keep that room.
 */
export function useTheirHead(
  table: RefObject<HTMLElement | null>,
  onHead: (px: number) => void,
  key: unknown,
): void {
  useLayoutEffect(() => {
    const area = table.current?.querySelector<HTMLElement>('[data-area="opponents"]')
    if (area == null) return undefined
    const observer = typeof ResizeObserver === 'function' ? new ResizeObserver(() => publish()) : null
    const watched = new Set<Element>()
    const publish = (): void => {
      const zones = [...area.querySelectorAll<HTMLElement>('[data-slot="battlefield-zone"]')]
      for (const zone of zones) {
        if (watched.has(zone)) continue
        watched.add(zone)
        observer?.observe(zone)
      }
      onHead(
        headOf(
          area.getBoundingClientRect().top,
          zones.map((zone) => zone.getBoundingClientRect().top),
        ),
      )
    }
    observer?.observe(area)
    publish()
    return () => observer?.disconnect()
  }, [table, onHead, key])
}
