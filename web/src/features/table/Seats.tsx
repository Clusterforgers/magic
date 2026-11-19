import { memo, useContext, useMemo, useRef, useState } from 'react'
import type { CSSProperties } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { describeLoss } from '@/features/game/outcome'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { PlayerId } from '@/protocol/generated'
import { Battlefield } from './Battlefield'
import { BoardZones } from './BoardZones'
import { useDefenderClick } from './combat/defender'
import { LandFoldContext } from './landFold'
import { LandsShownContext, landsShownBy, useLandsChoice } from './landPrefs'
import { crowdFoldsLands, gridPodsOneRow } from './boardSplit'
import { LandSummary } from './LandSummary'
import { BoardZoomContext, useZoomClose } from './boardZoom'
import type { CardSize } from './cardSize'
import { POD_COLUMN_PX } from './layout'
import type { PodPlace, SeatPlan } from './layout'
import { useSeatNamer } from './names'
import { SeatDot } from './PodParts'
import { playerOf } from './selectors'
import { PodRoomContext } from './podRoom'
import { SeatPanel } from './SeatPanel'
import { SeatTile } from './SeatTile'
import { FOLD_LANDS_BY_DEFAULT, TableOptionsContext } from './tableOptions'
import { useBoardFade } from './useBoardFade'
import { useBoxHeight } from './useBoxHeight'
import { useEdgeFade } from './useEdgeFade'
import { useLeadInView } from './useLeadInView'
import { useContentBox } from './useZoneFit'
import { GridCardContext, useGridCard } from './gridCard'

interface SeatAreaProps {
  seat: PlayerId
  size: CardSize
  /** `near` is the viewer's own side, `far` an opponent's. */
  facing: 'near' | 'far'
  /** The pod beside the board as a column, above it as one line, or not drawn. */
  pod: PodPlace
  /** An opponent of a grid: its lands fold into its pod's lands chip (`LandFoldContext`). */
  foldLands?: boolean
  /** The board has no permanent and draws no zone of its own: the seat is its pod's line or its tile (boss mode). */
  collapsed?: boolean
  /** A board without a pod draws its zones and lands chip in a line at its top; false where its frame carries them (a hero beside yours). */
  zones?: boolean
  /** A line pod keeps its figures to one row (a grid of opponents). */
  oneRow?: boolean
}

/**
 * The pod of a seat as a column beside its board. Its corner draws pictures sized by the column's height and by the
 * cards `card` px wide of the board beside it; a column too short for the least picture scrolls its chips inside.
 */
function PodColumn({ seat, card }: { seat: PlayerId; card: number | null }) {
  const column = useRef<HTMLDivElement>(null)
  useEdgeFade(column)
  const box = useContentBox(column)
  const height = box?.height ?? null
  const room = useMemo(() => ({ height, card }), [height, card])
  return (
    <div
      ref={column}
      data-slot="pod-column"
      style={{ width: POD_COLUMN_PX } as CSSProperties}
      className="fade-scroll min-h-0 shrink-0"
    >
      <PodRoomContext.Provider value={room}>
        <SeatPanel seat={seat} />
      </PodRoomContext.Provider>
    </div>
  )
}

/** One seat: its pod and its battlefield, side by side or one over the other. */
export const SeatArea = memo(function SeatArea({
  seat,
  size,
  facing,
  pod,
  foldLands = false,
  collapsed = false,
  zones = true,
  oneRow = false,
}: SeatAreaProps) {
  const aside = pod === 'aside'
  // The creatures' card width of the board, which the pod's corner follows.
  const [card, setCard] = useState<number | null>(null)
  const defender = useDefenderClick(seat)
  // A board that carries the chip lays its lands out or folds them as the table does (`landPrefs.ts`), and its chip turns
  // that for this board alone until the table changes; a phone's board without a drawn pod carries the chip itself.
  const shown = useContext(LandsShownContext)
  const [turned, setTurned] = useState(false)
  const [seen, setSeen] = useState(shown)
  if (seen !== shown) {
    setSeen(shown)
    setTurned(false)
  }
  const open = shown !== turned
  const fold = useMemo(
    () => (foldLands ? { open, toggle: () => setTurned((was) => !was) } : null),
    [foldLands, open],
  )
  const owner = useSeatNamer()(seat)
  return (
    <LandFoldContext value={fold}>
      <div
        data-slot="seat-area"
        data-seat-area={seat}
        data-defender-area={defender.state}
        onClick={defender.onClick}
        className={cn(
          'flex min-h-0 min-w-0 flex-1 gap-2',
          aside ? 'flex-row' : 'flex-col',
          defender.state !== undefined && 'cursor-pointer',
        )}
      >
        {pod === 'none' && zones ? (
          // The tile above the board has the figures; the zones and the lands chip are a line of their own at the board's top,
          // drawn over a board too tall for its frame, which overflows upward from its near edge.
          <div
            data-slot="board-lands"
            className="relative z-10 flex shrink-0 items-center justify-end gap-1 px-1 empty:hidden"
          >
            <BoardZones seat={seat} owner={owner} />
            <LandSummary seat={seat} owner={owner} density="tile" />
          </div>
        ) : null}
        {pod === 'aside' ? <PodColumn seat={seat} card={card} /> : null}
        {/* A line pod is on the table's middle side of its board: over an opponent's, under yours, by your hand. */}
        {pod === 'line' && facing === 'far' ? <SeatPanel seat={seat} variant="line" oneRow={oneRow} /> : null}
        {collapsed ? null : <Battlefield seat={seat} size={size} facing={facing} onCardWidth={setCard} />}
        {pod === 'line' && facing === 'near' ? (
          <SeatPanel seat={seat} variant="line" oneRow={oneRow} />
        ) : null}
      </div>
    </LandFoldContext>
  )
})

interface FocusBoardProps {
  seat: PlayerId
  size: CardSize
  pod: PodPlace
  /** The board folds its lands into a chip (a phone). */
  foldLands?: boolean
}

/**
 * The large opponent of focus mode. When the seat changes, the board that leaves stays in the same cell for
 * `FOCUS_FADE_MS`, fading out over the one that fades in (`focusBoard.css`); it takes no pointer and is hidden from
 * a screen reader and from the keyboard, so the board that is there answers at once. Each seat keeps its own board
 * through the change.
 */
function FocusBoard({ seat, size, pod, foldLands = false }: FocusBoardProps) {
  const { shown, leaving } = useBoardFade(seat)
  const layers = leaving === null ? [shown] : [shown, leaving]
  return (
    <div data-slot="focus-board" className="relative flex min-h-0 min-w-0 flex-1">
      {layers.map((layer) =>
        layer === shown ? (
          <div
            key={layer}
            data-slot="focus-layer"
            data-crossfade={leaving === null ? undefined : 'in'}
            className="flex min-h-0 min-w-0 flex-1"
          >
            <SeatArea seat={layer} size={size} facing="far" pod={pod} foldLands={foldLands} />
          </div>
        ) : (
          <div
            key={layer}
            data-slot="focus-layer"
            data-crossfade="out"
            aria-hidden="true"
            inert
            className="pointer-events-none absolute inset-0 flex"
          >
            <SeatArea seat={layer} size={size} facing="far" pod={pod} foldLands={foldLands} />
          </div>
        ),
      )}
    </div>
  )
}

interface TileStripProps {
  /** The opponents' seats, the one whose turn it is first. */
  tiles: readonly PlayerId[]
  /** The opponent whose board is large. */
  focused: PlayerId | null
  onFocusSeat: (seat: PlayerId) => void
  /** The strip's name for a screen reader; "Opponents" when left out. */
  label?: string
}

/** The strip of tiles of focus mode; it scrolls back to its start when the seat that leads it changes. */
export function TileStrip({ tiles, focused, onFocusSeat, label = 'Opponents' }: TileStripProps) {
  const strip = useRef<HTMLUListElement>(null)
  useLeadInView(strip, tiles[0])
  return (
    <ul
      ref={strip}
      role="list"
      aria-label={label}
      data-slot="tile-strip"
      className="relative flex h-14 shrink-0 gap-2 overflow-x-auto p-1 [scrollbar-width:none]"
    >
      {tiles.map((seat) => (
        <li key={seat} className="flex min-w-44 flex-1">
          <SeatTile seat={seat} focused={seat === focused} onFocusSeat={onFocusSeat} />
        </li>
      ))}
    </ul>
  )
}

// Whole class names, so Tailwind can see them.
const COLUMNS: Record<SeatPlan['columns'], string> = {
  1: 'grid-cols-1',
  2: 'grid-cols-2',
  3: 'grid-cols-3',
}

interface OpponentsProps {
  plan: SeatPlan
  size: CardSize
  /** The pod of the board drawn large (a wide board's, or the focused opponent's). */
  pod: PodPlace
  onFocusSeat: (seat: PlayerId) => void
  /** The large board folds its lands into a chip (a phone; a grid folds its own). */
  foldLands?: boolean
}

/**
 * The opponents' area. Grid mode: one opponent is a wide board beside a pod column, more are two or three columns of
 * a one-line pod over a board. Focus mode: a strip of tiles, two lines high, and the focused opponent's board below.
 */
export function Opponents({ plan, size, pod, onFocusSeat, foldLands = false }: OpponentsProps) {
  if (plan.shape === 'focus') {
    return (
      <div data-layout="focus" className="flex min-h-0 min-w-0 flex-col gap-2">
        <TileStrip tiles={plan.tiles} focused={plan.focused} onFocusSeat={onFocusSeat} />
        {plan.focused === null ? null : (
          <FocusBoard seat={plan.focused} size={size} pod={pod} foldLands={foldLands} />
        )}
      </div>
    )
  }
  if (plan.shape === 'duel') {
    const [seat] = plan.boards
    return (
      <div data-layout="grid" data-columns={1} className="flex min-h-0 min-w-0">
        {seat === undefined ? null : (
          <SeatArea seat={seat} size={size} facing="far" pod={pod} foldLands={foldLands} />
        )}
      </div>
    )
  }
  return <OpponentGrid plan={plan} size={size} />
}

/** The seat's result as a thin strip: its name and how it left the game. */
export function LeftStrip({ seat }: { seat: PlayerId }) {
  const owner = useSeatNamer()(seat)
  const lost = usePlay((state) => playerOf(state, seat)?.lost ?? null)
  return (
    <div
      data-slot="left-strip"
      data-seat-area={seat}
      className="flex min-w-0 items-center gap-2 rounded-lg border-2 border-destructive/40 bg-muted/40 px-2 py-0.5 text-xs"
    >
      <SeatDot seat={seat} />
      <span className="truncate font-semibold">{owner}</span>
      <span className="truncate text-destructive">{lost === null ? '' : describeLoss(lost)}</span>
    </div>
  )
}

/**
 * The boards of several opponents, each in a bordered frame, all at one card size (`gridCard.ts`). A seat that left the game is a thin strip over the
 * grid, so the seats still playing share its space.
 */
function OpponentGrid({ plan, size }: { plan: SeatPlan; size: CardSize }) {
  const gone = usePlay(
    useShallow((state) => plan.boards.filter((seat) => playerOf(state, seat)?.lost != null)),
  )
  const playing = plan.boards.filter((seat) => !gone.includes(seat))
  const columns = Math.max(1, Math.min(plan.columns, playing.length)) as SeatPlan['columns']
  const card = useGridCard(playing)
  const owner = useSeatNamer()
  const foldLands = useContext(TableOptionsContext).foldLands ?? FOLD_LANDS_BY_DEFAULT
  // A grid of more than one row of frames folds its lands until the viewer chooses, as a phone does (`crowdFoldsLands`),
  // and keeps each pod two lines high, the lands first, so every board under them keeps a row of cards.
  const crowded = crowdFoldsLands(plan)
  const shown = landsShownBy(useLandsChoice(), crowded)
  const [chosen, setZoomed] = useState<PlayerId | null>(null)
  // A seat that left the game, or a grid of one, has nothing to enlarge.
  const zoomed = chosen !== null && playing.includes(chosen) && playing.length > 1 ? chosen : null
  const zoom = useMemo(() => ({ zoomed, setZoomed }), [zoomed])
  const row = useRef<HTMLDivElement>(null)
  useZoomClose(zoom, row)
  // So do the pods of frames too low for a wrapped pod over a row of cards (a short window, `gridPodsOneRow`).
  const grid = useRef<HTMLDivElement>(null)
  const oneRow = gridPodsOneRow(plan, playing.length, useBoxHeight(grid))
  return (
    <BoardZoomContext value={zoom}>
      <LandsShownContext value={shown}>
        <GridCardContext value={card}>
          <div ref={row} className="flex min-h-0 min-w-0 flex-col gap-2">
            {gone.map((seat) => (
              <LeftStrip key={seat} seat={seat} />
            ))}
            {zoomed === null ? null : (
              // The enlarged board takes the whole row at its own card size; the other boards wait behind the close button.
              <div
                data-slot="seat-frame"
                data-zoomed=""
                role="region"
                aria-label={`${owner(zoomed)}'s board, enlarged`}
                className="flex min-h-0 min-w-0 flex-1 rounded-xl border-2 border-brass bg-felt-rim/40 p-1 shadow-e3"
              >
                <GridCardContext value={null}>
                  <SeatArea seat={zoomed} size={size} facing="far" pod="line" foldLands={foldLands} />
                </GridCardContext>
              </div>
            )}
            <div
              ref={grid}
              data-layout="grid"
              data-columns={columns}
              hidden={zoomed !== null}
              className={cn('grid min-h-0 min-w-0 flex-1 auto-rows-fr gap-2', COLUMNS[columns])}
            >
              {(zoomed === null ? playing : []).map((seat) => (
                <div
                  key={seat}
                  data-slot="seat-frame"
                  className="flex min-h-0 min-w-0 rounded-xl border border-brass/50 bg-felt-rim/30 p-1 shadow-[inset_0_2px_12px_oklch(0_0_0/0.35)]"
                >
                  <SeatArea
                    seat={seat}
                    size={size}
                    facing="far"
                    pod="line"
                    foldLands={foldLands}
                    oneRow={oneRow}
                  />
                </div>
              ))}
            </div>
          </div>
        </GridCardContext>
      </LandsShownContext>
    </BoardZoomContext>
  )
}
