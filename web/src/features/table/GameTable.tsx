import { memo, useContext, useMemo, useState } from 'react'
import type { CSSProperties } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { useFullBleed } from '@/components/appChrome'
import { usePlay } from '@/features/game/gameContext'
import type { PlayerId } from '@/protocol/generated'
import type { CardSize } from './cardSize'
import { CardPreview } from './CardPreview'
import { CombatOverlay } from './combat/CombatOverlay'
import { CombatProviders } from './combat/CombatProviders'
import { ControlsDock, ControlsSlotProvider } from './ControlsDock'
import { FxContext } from './fx/hooks'
import { EFFECTS_BY_DEFAULT, useTableFx } from './fx/useTableFx'
import { HandDock } from './HandDock'
import { ManualMenuProvider } from './manual/ManualMenu'
import { planTable, POD_COLUMN_PX, ZONE_GAP_PX } from './layout'
import type { TableMode } from './layout'
import { createMarksStore, MarksContext } from './prompt/marks'
import { createPreviewStore, PreviewContext } from './preview'
import { Opponents, SeatArea } from './Seats'
import { defaultFocusOf, seatCountOf } from './selectors'
import { SharedStrip } from './SharedStrip'
import { SideProvider } from './sideState'
import { SideSheet, SideTabs } from './SideTabs'
import { TableGrid } from './TableGrid'
import { TableOptionsContext } from './tableOptions'
import { TopBar } from './TopBar'
import { useViewport } from './useViewport'

/**
 * The table: exactly the window, never scrolling, in the regions of `TableGrid`. A top bar with the turn badge and the
 * menu; your opponents above and your board below a shared strip with the stack and the prompt; your hand along the
 * bottom edge with the pass controls at its right; the log, chat and stack beside it. Up to 4 seats it opens in grid
 * mode if a board column is at least 280 px wide, otherwise in focus mode, and the menu switches either. It reads the
 * game with `usePlay` and acts through `useGame().session`.
 */
const Table = memo(function Table() {
  const options = useContext(TableOptionsContext)
  const viewport = useViewport(options.viewport)
  useFullBleed()
  const { seat, seatCount, ready } = usePlay(
    useShallow((state) => ({ seat: state.seat, seatCount: seatCountOf(state), ready: state.view !== null })),
  )
  const followed = usePlay(defaultFocusOf)
  const active = usePlay((state) => state.view?.turn.active ?? null)
  const [mode, setMode] = useState<TableMode | undefined>(options.mode)
  const [chosen, setChosen] = useState<PlayerId | null>(options.focus ?? null)
  const [previews] = useState(createPreviewStore)
  const [marks] = useState(createMarksStore)
  const table = useMemo(
    () => planTable({ viewport, seatCount, viewer: seat ?? 0, mode, focus: chosen ?? followed, active }),
    [viewport, seatCount, seat, mode, chosen, followed, active],
  )
  const { regions, seats: plan } = table
  const size: CardSize = plan.compact ? 'compact' : 'normal'
  const fx = useTableFx(options.effects ?? EFFECTS_BY_DEFAULT, options.effectsSlow)

  return (
    <FxContext value={fx}>
      <PreviewContext value={previews}>
        <MarksContext value={marks}>
          <ManualMenuProvider>
            <ControlsSlotProvider compact={regions.phone}>
              <section
                aria-label="Game table"
                data-mode={plan.mode}
                className="relative h-dvh w-full overflow-hidden bg-table"
                style={fx.slow === 1 ? undefined : ({ '--fx-slow': fx.slow } as CSSProperties)}
              >
                <h1 className="sr-only">Game table</h1>
                {!ready || seat === null ? (
                  <p className="p-6 text-sm text-muted-foreground">Waiting for the table…</p>
                ) : (
                  <SideProvider kind={regions.side}>
                    <TableGrid
                      regions={regions}
                      stopsRow={table.stopsRow}
                      bar={
                        <TopBar
                          mode={plan.mode}
                          onMode={setMode}
                          pinnedFocus={chosen !== null}
                          onFollow={() => setChosen(null)}
                          stopsInMenu={false}
                          compact={regions.landscape}
                        />
                      }
                      opponents={
                        <Opponents plan={plan} size={size} pod={table.opponentPod} onFocusSeat={setChosen} />
                      }
                      shared={
                        <SharedStrip
                          sheetMax={regions.sheet}
                          inset={table.pod === 'aside' ? POD_COLUMN_PX + ZONE_GAP_PX : 0}
                        />
                      }
                      own={<SeatArea seat={plan.viewer} size={size} facing="near" pod={table.pod} />}
                      hand={<HandDock seat={plan.viewer} />}
                      controls={<ControlsDock stopsRow={table.stopsRow} />}
                      side={<SideTabs />}
                      // Inside the grid's own stacking context, so the lines run under the shared strip and above the boards.
                      overlay={<CombatOverlay />}
                    />
                    <SideSheet top={regions.landscape ? 'top-8' : 'top-11'} />
                  </SideProvider>
                )}
              </section>
            </ControlsSlotProvider>
            <CardPreview />
          </ManualMenuProvider>
        </MarksContext>
      </PreviewContext>
    </FxContext>
  )
})

/** The table inside the stores its combat marks share (`combat/CombatProviders`). */
export const GameTable = memo(function GameTable() {
  return (
    <CombatProviders>
      <Table />
    </CombatProviders>
  )
})
