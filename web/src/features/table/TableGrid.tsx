import type { CSSProperties, ReactNode } from 'react'
import { cn } from '@/lib/utils'
import { gridColumns, gridRows, mainColumns } from './layout'
import type { Regions } from './layout'

/** The named regions of the table, top to bottom: the bar, the opponents, the shared strip, your board, the hand and the controls. */
const AREAS = '"bar bar" "opponents opponents" "shared shared" "own own" "hand controls"'

interface TableGridProps {
  regions: Regions
  /** The controls dock draws the stop ladder as one row, which makes it wider. */
  stopsRow: boolean
  /** The top bar. */
  bar: ReactNode
  /** The opponents' area. */
  opponents: ReactNode
  /** The shared strip: stack chips and the prompt line. */
  shared: ReactNode
  /** Your board: your pod and your battlefield. */
  own: ReactNode
  /** The hand dock. */
  hand: ReactNode
  /** The controls dock, bottom right. */
  controls: ReactNode
  /** The side column or rail; nothing where the side area is a sheet. */
  side: ReactNode
  /** Drawn over the regions inside the grid's stacking context, such as the combat lines. */
  overlay?: ReactNode
}

// A one-cell grid whose track may shrink below its content (`minmax(0, 1fr)`), so the region's content is cut or scrolls
// inside the region instead of pushing the row open.
function Area({ area, className, children }: { area: string; className?: string; children: ReactNode }) {
  return (
    <div
      data-area={area}
      style={{ gridArea: area }}
      className={cn('grid min-h-0 min-w-0 grid-cols-[minmax(0,1fr)] grid-rows-[minmax(0,1fr)]', className)}
    >
      {children}
    </div>
  )
}

/**
 * The frame of the table, exactly the window high: a main area of five rows (bar 44 px, opponents `1fr`, shared
 * strip, your board `1.15fr`, hand dock beside the controls dock) and, from L up, the side column to its right.
 * Every region has `min-height: 0`, so a zone that cannot fit scrolls inside itself and the page never does.
 */
export function TableGrid({
  regions,
  stopsRow,
  bar,
  opponents,
  shared,
  own,
  hand,
  controls,
  side,
  overlay,
}: TableGridProps) {
  return (
    <div
      data-slot="table-grid"
      data-breakpoint={regions.breakpoint}
      data-side={regions.side}
      style={{ gridTemplateColumns: gridColumns(regions.side) } as CSSProperties}
      className="relative isolate grid h-full min-h-0 w-full"
    >
      <div
        data-slot="table-main"
        style={
          {
            gridTemplateRows: gridRows(regions),
            gridTemplateColumns: mainColumns(regions, stopsRow),
            gridTemplateAreas: AREAS,
          } as CSSProperties
        }
        className="grid min-h-0 min-w-0 bg-table"
      >
        <Area area="bar">{bar}</Area>
        <Area area="opponents" className="p-2">
          {opponents}
        </Area>
        <Area area="shared" className="px-2">
          {shared}
        </Area>
        <Area area="own" className="p-2">
          {own}
        </Area>
        <Area area="hand" className="relative z-30">
          {hand}
        </Area>
        <Area area="controls" className="relative z-30">
          {controls}
        </Area>
      </div>
      {side}
      {overlay}
    </div>
  )
}
