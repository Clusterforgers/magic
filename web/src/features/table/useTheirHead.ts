/** Measures how far down a row of the opponents' area their battlefields begin: what their pods and tile strip take. */
import { useLayoutEffect } from 'react'
import type { RefObject } from 'react'

/** A battlefield zone's top and, in a grid of frames, the top of its seat frame, in px of the page. */
export interface ZoneTop {
  top: number
  frame?: number | undefined
}

const finiteOr = (value: number | undefined, fallback: number): number =>
  value !== undefined && Number.isFinite(value) ? value : fallback

/**
 * The deepest head of the opponents' battlefields below the top of their area, in px; 0 without a zone. A zone in a
 * seat frame counts from its own frame's top, moved up to the first row of frames: in a grid of several rows the head
 * of a lower row is its pod line, never the rows above it, so the head does not follow the rows' heights (which the
 * split sets from it).
 */
export function headOf(areaTop: number, zones: readonly (number | ZoneTop)[]): number {
  const tops = zones.map((zone) => (typeof zone === 'number' ? { top: zone } : zone))
  const frames = tops.map((zone) => finiteOr(zone.frame, Number.POSITIVE_INFINITY))
  const firstRow = Math.min(...frames)
  return Math.max(
    0,
    ...tops.map((zone, index) => {
      if (!Number.isFinite(zone.top)) return 0
      const frame = frames[index] ?? Number.POSITIVE_INFINITY
      return Number.isFinite(frame) ? zone.top - frame + firstRow - areaTop : zone.top - areaTop
    }),
  )
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
          zones.map((zone) => ({
            top: zone.getBoundingClientRect().top,
            frame: zone.closest('[data-slot="seat-frame"]')?.getBoundingClientRect().top,
          })),
        ),
      )
    }
    observer?.observe(area)
    publish()
    return () => observer?.disconnect()
  }, [table, onHead, key])
}
