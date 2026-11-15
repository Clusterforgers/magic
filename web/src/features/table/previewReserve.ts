/** The part of the window a pinned card preview takes over the boards, which every board keeps its cards out of. */
import { useLayoutEffect, useMemo, useState, useSyncExternalStore } from 'react'
import type { RefObject } from 'react'
import { anchorOf } from './preview'
import type { Anchor, PreviewTarget } from './preview'
import { GAP_PX } from './previewFit'
import type { PreviewFit } from './previewFit'
import { boxOf, fitClear, meets, panelBox } from './previewClear'
import type { Box, ClearBox } from './previewClear'

/** The least width in px a board keeps for its cards beside a pinned preview; past it the preview overlays the rest. */
export const ZONE_KEEP_PX = 240

/** The padding in px a board's zone adds on each side to keep its cards out of the preview. */
export interface BoardInset {
  left: number
  right: number
}

/** No padding. */
export const NO_INSET: BoardInset = { left: 0, right: 0 }

/**
 * The inset of a board's zone at `zone` (window px) that keeps its cards `GAP_PX` clear of the preview's panel at
 * `panel`: on the side the panel stands nearer to, and only where the two meet top to bottom and side to side. A zone
 * keeps `ZONE_KEEP_PX` (or its own width where less) however wide the panel.
 */
export function boardInset(zone: Box, panel: Box | null): BoardInset {
  if (panel === null) return NO_INSET
  const width = zone.right - zone.left
  if (width <= 0 || panel.top >= zone.bottom || panel.bottom <= zone.top) return NO_INSET
  if (panel.left >= zone.right || panel.right <= zone.left) return NO_INSET
  const most = Math.max(0, width - Math.min(width, ZONE_KEEP_PX))
  const fromRight = panel.left + panel.right > zone.left + zone.right
  const cover = fromRight ? zone.right - panel.left + GAP_PX : panel.right - zone.left + GAP_PX
  const inset = Math.ceil(Math.min(most, Math.max(0, cover)))
  return fromRight ? { left: 0, right: inset } : { left: inset, right: 0 }
}

let reserved: Box | null = null
const listeners = new Set<() => void>()

/** Publishes the panel of a pinned preview that lies over the boards, or null once it is gone or lies clear of them. */
export function reservePreview(next: Box | null): void {
  const same =
    next === reserved ||
    (next !== null &&
      reserved !== null &&
      next.left === reserved.left &&
      next.right === reserved.right &&
      next.top === reserved.top &&
      next.bottom === reserved.bottom)
  if (same) return
  reserved = next
  for (const listener of listeners) listener()
}

const subscribe = (listener: () => void): (() => void) => {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

/** The panel the boards keep their cards out of, or null. */
export function useReservedPreview(): Box | null {
  return useSyncExternalStore(
    subscribe,
    () => reserved,
    () => null,
  )
}

/** The boxes of the cards on every battlefield, in window px. */
export function boardCardBoxes(): Anchor[] {
  return [...document.querySelectorAll('[data-battlefield-seat] [data-object-id]')].map(anchorOf)
}

const sideDock = (fit: PreviewFit): boolean => fit.dock === 'left' || fit.dock === 'right'

/**
 * Where the preview of `target` stands (`fitClear`). A hovered card's panel looks for empty room beside the board's
 * cards each time it draws. A pinned one is placed once for its card, window and `clear`: over empty room where there
 * is some, and otherwise its panel is reserved (`reservePreview`), so the boards it lies over narrow beside it.
 */
export function usePreviewFit(target: PreviewTarget, clear: readonly ClearBox[]): PreviewFit {
  const width = window.innerWidth
  const height = window.innerHeight
  const placed = useMemo(() => {
    if (!target.pinned) return null
    const viewport = { width, height }
    // While boards stand narrowed their cards are clear of the panel already: keep the panel where it is.
    const narrowed = reserved !== null
    const fit = fitClear(target.anchor, viewport, clear, narrowed ? [] : boardCardBoxes())
    const panel = sideDock(fit) ? panelBox(fit, viewport) : null
    const covers = panel !== null && (narrowed || boardCardBoxes().some((card) => meets(panel, boxOf(card))))
    return { fit, reserve: covers ? panel : null }
  }, [target, width, height, clear])
  useLayoutEffect(() => reservePreview(placed?.reserve ?? null), [placed])
  useLayoutEffect(() => () => reservePreview(null), [])
  return placed?.fit ?? fitClear(target.anchor, { width, height }, clear, boardCardBoxes())
}

const sameInset = (a: BoardInset, b: BoardInset): boolean => a.left === b.left && a.right === b.right

/** The inset of the board zone in `ref` beside the reserved preview (`boardInset`), measured again as the zone resizes. */
export function usePreviewInset(ref: RefObject<HTMLElement | null>): BoardInset {
  const panel = useReservedPreview()
  const [inset, setInset] = useState<BoardInset>(NO_INSET)
  useLayoutEffect(() => {
    const zone = ref.current
    if (zone === null) return
    // The zone's own box: its padding, which this inset sets, never moves it.
    const measure = (): void => {
      const rect = zone.getBoundingClientRect()
      const next = boardInset(
        { left: rect.left, top: rect.top, right: rect.right, bottom: rect.bottom },
        panel,
      )
      setInset((was) => (sameInset(was, next) ? was : next))
    }
    measure()
    if (panel === null || typeof ResizeObserver !== 'function') return
    const observer = new ResizeObserver(measure)
    observer.observe(zone)
    window.addEventListener('resize', measure)
    return () => {
      observer.disconnect()
      window.removeEventListener('resize', measure)
    }
  }, [ref, panel])
  return inset
}
