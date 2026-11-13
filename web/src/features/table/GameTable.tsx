import { memo, useCallback, useContext, useMemo, useRef, useState } from 'react'
import type { CSSProperties } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { useFullBleed } from '@/components/appChrome'
import { useGame, usePlay } from '@/features/game/gameContext'
import { SOUNDS_BY_DEFAULT, useSoundCues } from '@/features/sound/useSoundCues'
import type { PlayerId } from '@/protocol/generated'
import type { CardSize } from './cardSize'
import { CardPreview } from './CardPreview'
import { CombatOverlay } from './combat/CombatOverlay'
import { CombatProviders } from './combat/CombatProviders'
import { ControlsDock, ControlsSlotProvider } from './ControlsDock'
import { FxContext } from './fx/hooks'
import { FxTravel } from './fx/FxTravel'
import { FxStrikes } from './fx/FxStrikes'
import { EFFECTS_BY_DEFAULT, useTableFx } from './fx/useTableFx'
import { HandDock } from './HandDock'
import { MomentsContext } from './moment/store'
import { TableMoments } from './moment/TableMoments'
import { useTableMoments } from './moment/useTableMoments'
import { LexiconDialog } from './lexicon/LexiconDialog'
import { ManualMenuProvider } from './manual/ManualMenu'
import {
  alliesBandPx,
  handGrowMost,
  openingGrowMost,
  planTable,
  POD_COLUMN_PX,
  sidePanelRows,
  ZONE_GAP_PX,
} from './layout'
import type { TableMode } from './layout'
import { createMarksStore, MarksContext } from './prompt/marks'
import { promptInStrip } from './prompt/ownDecision'
import { createPreviewStore, PreviewContext } from './preview'
import { Opponents, SeatArea } from './Seats'
import { BossSide, PartySide } from './BossLayout'
import { bossSidesKey, defaultFocusOf, handSeat, parseBossSides, seatCountOf, turnLeft } from './selectors'
import { SharedStrip } from './SharedStrip'
import { SideProvider } from './sideState'
import { StopControlsProvider } from './stops/stopControls'
import { SidePanelMount, SideSheet, SideTabs } from './SideTabs'
import { TableGrid } from './TableGrid'
import { useBoardSplit } from './useBoardSplit'
import { useTheirHead } from './useTheirHead'
import { sheetReserve } from './boardSplit'
import { TableOptionsContext } from './tableOptions'
import { TopBar } from './TopBar'
import { TableTour } from './tour/TableTour'
import { useViewport } from './useViewport'

/**
 * The table: exactly the window, never scrolling, in the regions of `TableGrid`. A top bar with the turn badge and the
 * menu; your opponents above and your board below a shared strip with the stack and the prompt; your hand along the
 * bottom edge with the pass controls at its right; the log, chat and stack beside it. Up to 4 seats it opens in grid
 * mode if a board column is at least 280 px wide, otherwise in focus mode, and the menu switches either. It reads the
 * game with `usePlay` and acts through `useGame().session`.
 */
/** In place of the pass controls for a spectator, who has nothing to answer. */
function Watching() {
  return (
    <p
      data-slot="watching"
      className="flex h-full items-center justify-center px-3 text-center text-sm text-muted-foreground"
    >
      Watching: you hold no seat
    </p>
  )
}

const Table = memo(function Table() {
  const options = useContext(TableOptionsContext)
  const viewport = useViewport(options.viewport)
  useFullBleed()
  useSoundCues(useGame().session.store, options.sounds ?? SOUNDS_BY_DEFAULT)
  const { seat, seatCount, ready, spectator } = usePlay(
    useShallow((state) => ({
      seat: state.seat,
      seatCount: seatCountOf(state),
      ready: state.view !== null,
      spectator: state.view?.spectator === true,
    })),
  )
  // A spectator holds no seat, so the table is drawn from the first seat's side, its hand face down.
  const viewer = spectator ? 0 : (seat ?? 0)
  const followed = usePlay(defaultFocusOf)
  const active = usePlay((state) => (turnLeft(state) ? null : (state.view?.turn.active ?? null)))
  const lowered = usePlay(promptInStrip)
  // The hand of the player the viewer controls this turn, else the viewer's own (CR 722).
  const handFrom = usePlay((state) => handSeat(state, viewer))
  const [mode, setMode] = useState<TableMode | undefined>(options.mode)
  const [chosen, setChosen] = useState<PlayerId | null>(options.focus ?? null)
  const [previews] = useState(createPreviewStore)
  const [marks] = useState(createMarksStore)
  // A fight against a boss opens in boss mode: the boss's side on top, the party together at the bottom.
  const sidesKey = usePlay(bossSidesKey)
  const sides = useMemo(() => parseBossSides(sidesKey), [sidesKey])
  const table = useMemo(
    () => planTable({ viewport, seatCount, viewer, mode, focus: chosen ?? followed, active, sides }),
    [viewport, seatCount, viewer, mode, chosen, followed, active, sides],
  )
  const { seats: plan } = table
  // The overhang the sheet wants in steps of 8 px: the split reserves part of it, and a pod column keeps all of it free.
  const [overhang, setOverhang] = useState(0)
  const onOverhang = useCallback((px: number) => setOverhang(Math.ceil(Math.max(0, px) / 8) * 8), [])
  const [head, setHead] = useState(0)
  // Steps of 8 px, so a pod that grows by a pixel does not move the boards.
  const onHead = useCallback((px: number) => setHead(Math.ceil(px / 8) * 8), [])
  const section = useRef<HTMLElement>(null)
  useTheirHead(section, onHead, `${ready}|${plan.shape}|${plan.boards.join(',')}`)
  const regions = useBoardSplit(table.regions, plan, sheetReserve(overhang), head, overhang)
  const size: CardSize = plan.compact ? 'compact' : 'normal'
  const fx = useTableFx(options.effects ?? EFFECTS_BY_DEFAULT, options.effectsSlow)
  const moments = useTableMoments(options.effects ?? EFFECTS_BY_DEFAULT)

  return (
    <FxContext value={fx}>
      <MomentsContext value={moments}>
        <PreviewContext value={previews}>
          <MarksContext value={marks}>
            <ManualMenuProvider>
              <StopControlsProvider inBar={table.ladderInBar}>
                <ControlsSlotProvider form={table.dock}>
                  <section
                    ref={section}
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
                              bossFight={sides !== null}
                              pinnedFocus={chosen !== null}
                              onFollow={() => setChosen(null)}
                              stopsInMenu={false}
                              compact={regions.landscape}
                            />
                          }
                          opponents={
                            plan.shape === 'boss' ? (
                              <BossSide
                                plan={plan}
                                size={size}
                                pod={table.opponentPod}
                                onFocusSeat={setChosen}
                              />
                            ) : (
                              <Opponents
                                plan={plan}
                                size={size}
                                pod={table.opponentPod}
                                onFocusSeat={setChosen}
                              />
                            )
                          }
                          shared={
                            <SharedStrip
                              sheetMax={regions.sheet}
                              onOverhang={onOverhang}
                              inset={table.pod === 'aside' ? POD_COLUMN_PX + ZONE_GAP_PX : 0}
                            />
                          }
                          own={
                            plan.shape === 'boss' ? (
                              <PartySide
                                plan={plan}
                                size={size}
                                pod={table.pod}
                                band={alliesBandPx(regions, plan.allies.length)}
                                phone={regions.phone}
                              />
                            ) : (
                              <SeatArea seat={plan.viewer} size={size} facing="near" pod={table.pod} />
                            )
                          }
                          hand={
                            <HandDock
                              seat={handFrom}
                              growMost={handGrowMost(regions)}
                              openingMost={openingGrowMost(regions)}
                            />
                          }
                          controls={spectator ? <Watching /> : <ControlsDock stopsRow={table.stopsRow} />}
                          side={<SideTabs />}
                          sidePanel={
                            <SidePanelMount rows={sidePanelRows(regions, lowered)} lowered={lowered} />
                          }
                          // Inside the grid's own stacking context, so the lines run under the shared strip and above the boards.
                          overlay={<CombatOverlay />}
                        />
                        <SideSheet />
                        <TableTour />
                      </SideProvider>
                    )}
                    <FxTravel />
                    <FxStrikes />
                  </section>
                </ControlsSlotProvider>
              </StopControlsProvider>
              <CardPreview />
              <TableMoments />
              <LexiconDialog />
            </ManualMenuProvider>
          </MarksContext>
        </PreviewContext>
      </MomentsContext>
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
