import type { CSSProperties, ReactNode, Ref } from 'react'
import { cn } from '@/lib/utils'
import { CHOICE_AREAS, gridColumns, gridRows, mainColumns, TABLE_AREAS } from './layout'
import type { Regions } from './layout'
import { usePhase } from './turn/usePhase'

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
  /** The overlay of the main area that an open rail panel or sheet is drawn into (`SidePanelMount`). */
  sidePanel?: ReactNode
  /** Drawn over the regions inside the grid's stacking context, such as the combat lines. */
  overlay?: ReactNode
  /** The decision's detail as a sheet over the hand, where no column holds it (`prompt/detail/DecisionSheet`). */
  sheet?: ReactNode
  /** The centre overlay of a card prompt (`prompt/choiceOverlay`): its element's ref and whether it is open. */
  choice?: { open: boolean; ref: Ref<HTMLDivElement> }
}

// A one-cell grid whose track may shrink below its content (`minmax(0, 1fr)`), so the region's content is cut or scrolls
// inside the region instead of pushing the row open.
function Area({
  area,
  className,
  style,
  children,
}: {
  area: string
  className?: string
  style?: CSSProperties
  children: ReactNode
}) {
  return (
    <div
      data-area={area}
      style={{ gridArea: area, ...style }}
      className={cn('grid min-h-0 min-w-0 grid-cols-[minmax(0,1fr)] grid-rows-[minmax(0,1fr)]', className)}
    >
      {children}
    </div>
  )
}

/**
 * The frame of the table, exactly the window high: a main area of five rows (bar 44 px, opponents, shared
 * strip, your board, hand dock beside the controls dock; the two boards split by `boardSplit.ts`) and, from L up, the
 * side column to its right.
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
  sidePanel,
  overlay,
  sheet,
  choice,
}: TableGridProps) {
  // Read here, so a change of phase redraws the frame alone: the regions are elements made by the table and stay as they were.
  const phase = usePhase()
  return (
    <div
      data-slot="table-grid"
      // The phase of the turn, for the stylesheet to theme the bar, the strip and the ground (`turn/phase.css`).
      data-phase={phase?.id}
      data-breakpoint={regions.breakpoint}
      data-side={regions.side}
      style={{ gridTemplateColumns: gridColumns(regions.side) } as CSSProperties}
      className="relative isolate grid h-full min-h-0 w-full"
    >
      <div
        data-slot="table-main"
        style={
          {
            gridTemplateRows: gridRows(regions, choice?.open === true),
            gridTemplateColumns: mainColumns(regions, stopsRow),
            gridTemplateAreas: choice?.open === true ? CHOICE_AREAS : TABLE_AREAS,
          } as CSSProperties
        }
        className="felt grid min-h-0 min-w-0"
      >
        <Area area="bar">{bar}</Area>
        {/* The prompt sheet grows up into the band's room this area keeps free below its boards for every prompt and between them, so it covers no card and moves no board. */}
        <Area
          area="opponents"
          className="px-2 pt-2"
          style={{ paddingBottom: `calc(0.5rem + ${Math.floor(regions.sheetRoom)}px)` }}
        >
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
        {choice === undefined ? null : (
          // Over both boards, which `CHOICE_AREAS` puts next to each other, down to the top of the strip's sheet.
          <div
            ref={choice.ref}
            data-slot="choice-host"
            hidden={!choice.open}
            style={{ gridRow: '2 / 4', gridColumn: '1 / -1' }}
            className="relative z-15 flex min-h-0 min-w-0 items-center justify-center bg-scrim px-2 pt-2 pb-[calc(0.125rem+var(--sheet-overhang,0px))]"
          />
        )}
        {sheet}
        {sidePanel}
      </div>
      {side}
      {overlay}
    </div>
  )
}
