/** One card size for a grid of opponents' boards: the least of the widths each board fits on its own. */
import { createContext, useCallback, useMemo, useState } from 'react'
import type { PlayerId } from '@/protocol/generated'

/** What a board of a grid reads and tells. */
export interface GridCard {
  /** The widest card a board of the grid draws, in px; null before every board was measured. */
  most: number | null
  /** Tells the grid the card width `seat`'s board fits on its own (null before it is measured). */
  report: (seat: PlayerId, width: number | null) => void
}

/** Outside a grid a board fits alone. */
export const GridCardContext = createContext<GridCard | null>(null)

/**
 * The shared card width of the boards `seats`: the least of their own fits. Each board reports the fit of its box
 * alone, which the shared width does not change, so the grid settles in one pass.
 */
export function gridMost(seats: readonly PlayerId[], widths: ReadonlyMap<PlayerId, number>): number | null {
  const known = seats.map((seat) => widths.get(seat)).filter((width) => width !== undefined)
  return known.length === 0 ? null : Math.min(...known)
}

/** The grid's context value for the boards `seats`. */
export function useGridCard(seats: readonly PlayerId[]): GridCard {
  const [widths, setWidths] = useState<ReadonlyMap<PlayerId, number>>(new Map())
  const report = useCallback((seat: PlayerId, width: number | null) => {
    setWidths((was) => {
      if ((was.get(seat) ?? null) === width) return was
      const next = new Map(was)
      if (width === null) next.delete(seat)
      else next.set(seat, width)
      return next
    })
  }, [])
  const most = gridMost(seats, widths)
  return useMemo(() => ({ most, report }), [most, report])
}
