import { memo, useContext, useMemo, useState } from 'react'
import type { CSSProperties } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { PlayerId } from '@/protocol/generated'
import { AutoPassSwitch } from './AutoPassSwitch'
import type { CardSize } from './cardSize'
import { CardPreview } from './CardPreview'
import { CombatOverlay } from './combat/CombatOverlay'
import { CombatProviders } from './combat/CombatProviders'
import { ConcedeButton } from './ConcedeButton'
import { CentreStrip } from './CentreStrip'
import { Drawer } from './Drawer'
import { FxContext } from './fx/hooks'
import { EFFECTS_BY_DEFAULT, useTableFx } from './fx/useTableFx'
import { arrangeSeats } from './layout'
import type { SeatPlan, TableMode } from './layout'
import { createMarksStore, MarksContext } from './prompt/marks'
import { createPreviewStore, PreviewContext } from './preview'
import { OpponentBoard, OwnBoard } from './SeatBoards'
import { SeatTile } from './SeatTile'
import { defaultFocusOf, seatCountOf } from './selectors'
import { StackPanel } from './StackPanel'
import { TableOptionsContext } from './tableOptions'
import { TableToolbar } from './TableToolbar'

// Whole class names, so Tailwind can see them.
const COLUMNS: Record<SeatPlan['columns'], string> = {
  1: 'grid-cols-1',
  2: 'grid-cols-1 md:grid-cols-2',
  3: 'grid-cols-1 md:grid-cols-2 xl:grid-cols-3',
}

interface OpponentsProps {
  plan: SeatPlan
  size: CardSize
  onFocusSeat: (seat: PlayerId) => void
}

/** Grid mode: every opponent's board in 1 to 3 columns. Focus mode: a tile for each and one board drawn large. */
function Opponents({ plan, size, onFocusSeat }: OpponentsProps) {
  if (plan.mode === 'grid') {
    return (
      <div
        data-layout="grid"
        data-columns={plan.columns}
        className={cn('grid items-start gap-3', COLUMNS[plan.columns])}
      >
        {plan.boards.map((seat) => (
          <OpponentBoard key={seat} seat={seat} size={size} />
        ))}
      </div>
    )
  }
  return (
    <div data-layout="focus" className="grid gap-3">
      <ul
        role="list"
        aria-label="Opponents"
        className="relative flex gap-2 overflow-x-auto p-1 sm:grid sm:grid-cols-[repeat(auto-fill,minmax(10rem,1fr))] sm:overflow-visible"
      >
        {plan.tiles.map((seat) => (
          <li key={seat} className="flex">
            <SeatTile seat={seat} focused={seat === plan.focused} onFocusSeat={onFocusSeat} />
          </li>
        ))}
      </ul>
      {plan.focused === null ? null : <OpponentBoard seat={plan.focused} size={size} />}
    </div>
  )
}

/**
 * The table: every seat's battlefield and panel with the viewer at the bottom, the turn banner and stack in the
 * middle, and the log and chat in a drawer. Up to 4 seats it opens in grid mode, from 5 in focus mode, and the
 * toolbar switches either. It reads the game with `usePlay` and acts through `useGame().session`.
 */
const Table = memo(function Table() {
  const options = useContext(TableOptionsContext)
  const { seat, seatCount, ready } = usePlay(
    useShallow((state) => ({ seat: state.seat, seatCount: seatCountOf(state), ready: state.view !== null })),
  )
  const followed = usePlay(defaultFocusOf)
  const [mode, setMode] = useState<TableMode | undefined>(options.mode)
  const [chosen, setChosen] = useState<PlayerId | null>(options.focus ?? null)
  const [previews] = useState(createPreviewStore)
  const [marks] = useState(createMarksStore)
  const plan = useMemo(
    () => arrangeSeats({ seatCount, viewer: seat ?? 0, mode, focus: chosen ?? followed }),
    [seatCount, seat, mode, chosen, followed],
  )
  const size: CardSize = plan.compact ? 'compact' : 'normal'
  const fx = useTableFx(options.effects ?? EFFECTS_BY_DEFAULT, options.effectsSlow)

  return (
    <FxContext value={fx}>
      <PreviewContext value={previews}>
        <MarksContext value={marks}>
          <section
            aria-label="Game table"
            className="flex w-full items-start"
            style={fx.slow === 1 ? undefined : ({ '--fx-slow': fx.slow } as CSSProperties)}
          >
            <div className="relative grid min-w-0 flex-1 content-start gap-3 p-3 max-lg:pb-16 lg:p-4">
              <h1 className="sr-only">Game table</h1>
              {!ready || seat === null ? (
                <p className="p-6 text-sm text-muted-foreground">Waiting for the table…</p>
              ) : (
                <>
                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <TableToolbar
                      mode={plan.mode}
                      onMode={setMode}
                      pinnedFocus={chosen !== null}
                      onFollow={() => setChosen(null)}
                    />
                    <div className="flex flex-wrap items-center gap-3">
                      <AutoPassSwitch />
                      <ConcedeButton />
                    </div>
                  </div>
                  <Opponents plan={plan} size={size} onFocusSeat={setChosen} />
                  {/* Above the combat lines, so they run behind the prompt and never over its words. */}
                  <div className="relative z-20 grid gap-3">
                    <CentreStrip />
                    <StackPanel />
                  </div>
                  <OwnBoard seat={plan.viewer} size={size} />
                  <CombatOverlay />
                </>
              )}
            </div>
            <Drawer />
          </section>
          <CardPreview />
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
