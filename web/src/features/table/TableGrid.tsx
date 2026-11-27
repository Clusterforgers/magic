import type { CSSProperties, ReactNode, Ref } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import { anchor } from '@/features/learn/anchors'
import type { CoachMark } from '@/features/learn/anchors'
import { gridColumns, gridRows, mainColumns, tableAreas } from './layout'
import type { Regions } from './layout'
import { openingPlace, useOpeningInOverlay } from './prompt/choiceOverlay'
import { kindKey } from './prompt/detail/asking'
import { isBoardChoice } from './prompt/home'
import { ownDecision } from './prompt/ownDecision'
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
  mark,
  children,
}: {
  area: string
  className?: string
  style?: CSSProperties
  /** The coach's anchor of the area. */
  mark?: CoachMark
  children: ReactNode
}) {
  return (
    <div
      {...mark}
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
  const boardChoice = usePlay((state) => {
    const own = ownDecision(state)
    const kind = own === null ? '' : kindKey(own.decision.kind)
    return isBoardChoice(kind) ? kind : null
  })
  // A choice of cards covers both boards, your board above the strip; the opening hand stands over the whole table and
  // dims the opponents and the strip only, so your board, your life and the hand dock stay in view while you decide.
  const opening = useOpeningInOverlay()
  const swap = choice?.open === true && !opening
  const place = openingPlace(regions.main.width)
  return (
    <div
      data-slot="table-grid"
      // The phase of the turn, for the stylesheet to theme the bar, the strip and the ground (`turn/phase.css`).
      data-phase={phase?.id}
      // A decision answered by pressing the table dims everything outside it (`promptBand.css`, `isBoardChoice`).
      data-board-choice={boardChoice ?? undefined}
      data-breakpoint={regions.breakpoint}
      data-side={regions.side}
      style={{ gridTemplateColumns: gridColumns(regions.side) } as CSSProperties}
      className="relative isolate grid h-full min-h-0 w-full"
    >
      <div
        data-slot="table-main"
        style={
          {
            gridTemplateRows: gridRows(regions, swap),
            gridTemplateColumns: mainColumns(regions, stopsRow),
            gridTemplateAreas: tableAreas(regions.phone, swap),
          } as CSSProperties
        }
        className="felt grid min-h-0 min-w-0"
      >
        <Area area="bar">{bar}</Area>
        <Area area="opponents" className="p-2" mark={anchor('opponents')}>
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
        {opening ? (
          // The opponents and the strip dim under the opening hand; your board, your pod and your life stay lit.
          <div
            aria-hidden="true"
            data-slot="opening-scrim"
            style={{ gridRow: '2 / 4', gridColumn: '1 / -1' }}
            className="pointer-events-none relative z-25 bg-scrim"
          />
        ) : null}
        {choice === undefined ? null : (
          // Over both boards, which `CHOICE_AREAS` puts next to each other, down to the top of the strip's sheet. The
          // opening hand stands over the whole table instead, centred and clear of your pod (`openingPlace`): the host
          // lets presses through to your board around its panel.
          <div
            ref={choice.ref}
            data-slot="choice-host"
            data-opening-place={opening ? (place.inset > 0 ? 'table' : 'top') : undefined}
            hidden={!choice.open}
            style={{
              gridRow: opening ? place.rows : '2 / 4',
              gridColumn: '1 / -1',
              ...(opening ? { paddingLeft: place.inset || 8, paddingRight: place.inset || 8 } : {}),
            }}
            className={cn(
              'relative flex min-h-0 min-w-0 items-center justify-center',
              opening
                ? 'pointer-events-none z-25 py-2 *:pointer-events-auto'
                : 'z-15 bg-scrim px-2 pt-2 pb-0.5',
            )}
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
