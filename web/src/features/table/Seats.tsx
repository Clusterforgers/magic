import { memo, useMemo, useRef, useState } from 'react'
import type { CSSProperties } from 'react'
import { cn } from '@/lib/utils'
import type { PlayerId } from '@/protocol/generated'
import { Battlefield } from './Battlefield'
import type { CardSize } from './cardSize'
import { POD_COLUMN_PX } from './layout'
import type { PodPlace, SeatPlan } from './layout'
import { PodRoomContext } from './podRoom'
import { SeatPanel } from './SeatPanel'
import { SeatTile } from './SeatTile'
import { useBoardFade } from './useBoardFade'
import { useEdgeFade } from './useEdgeFade'
import { useLeadInView } from './useLeadInView'
import { useContentBox } from './useZoneFit'

interface SeatAreaProps {
  seat: PlayerId
  size: CardSize
  /** `near` is the viewer's own side, `far` an opponent's. */
  facing: 'near' | 'far'
  /** The pod beside the board as a column, above it as one line, or not drawn. */
  pod: PodPlace
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
export const SeatArea = memo(function SeatArea({ seat, size, facing, pod }: SeatAreaProps) {
  const aside = pod === 'aside'
  // The creatures' card width of the board, which the pod's corner follows.
  const [card, setCard] = useState<number | null>(null)
  return (
    <div
      data-slot="seat-area"
      data-seat-area={seat}
      className={cn('flex min-h-0 min-w-0 flex-1 gap-2', aside ? 'flex-row' : 'flex-col')}
    >
      {pod === 'aside' ? <PodColumn seat={seat} card={card} /> : null}
      {/* A line pod is on the table's middle side of its board: over an opponent's, under yours, by your hand. */}
      {pod === 'line' && facing === 'far' ? <SeatPanel seat={seat} variant="line" /> : null}
      <Battlefield seat={seat} size={size} facing={facing} onCardWidth={setCard} />
      {pod === 'line' && facing === 'near' ? <SeatPanel seat={seat} variant="line" /> : null}
    </div>
  )
})

interface FocusBoardProps {
  seat: PlayerId
  size: CardSize
  pod: PodPlace
}

/**
 * The large opponent of focus mode. When the seat changes, the board that leaves stays in the same cell for
 * `FOCUS_FADE_MS`, fading out over the one that fades in (`focusBoard.css`); it takes no pointer and is hidden from
 * a screen reader and from the keyboard, so the board that is there answers at once. Each seat keeps its own board
 * through the change.
 */
function FocusBoard({ seat, size, pod }: FocusBoardProps) {
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
            <SeatArea seat={layer} size={size} facing="far" pod={pod} />
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
            <SeatArea seat={layer} size={size} facing="far" pod={pod} />
          </div>
        ),
      )}
    </div>
  )
}

interface TileStripProps {
  /** The opponents' seats, the one whose turn it is first. */
  tiles: PlayerId[]
  /** The opponent whose board is large. */
  focused: PlayerId | null
  onFocusSeat: (seat: PlayerId) => void
}

/** The strip of tiles of focus mode; it scrolls back to its start when the seat that leads it changes. */
function TileStrip({ tiles, focused, onFocusSeat }: TileStripProps) {
  const strip = useRef<HTMLUListElement>(null)
  useLeadInView(strip, tiles[0])
  return (
    <ul
      ref={strip}
      role="list"
      aria-label="Opponents"
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
}

/**
 * The opponents' area. Grid mode: one opponent is a wide board beside a pod column, more are two or three columns of
 * a one-line pod over a board. Focus mode: a strip of tiles, two lines high, and the focused opponent's board below.
 */
export function Opponents({ plan, size, pod, onFocusSeat }: OpponentsProps) {
  if (plan.shape === 'focus') {
    return (
      <div data-layout="focus" className="flex min-h-0 min-w-0 flex-col gap-2">
        <TileStrip tiles={plan.tiles} focused={plan.focused} onFocusSeat={onFocusSeat} />
        {plan.focused === null ? null : <FocusBoard seat={plan.focused} size={size} pod={pod} />}
      </div>
    )
  }
  if (plan.shape === 'duel') {
    const [seat] = plan.boards
    return (
      <div data-layout="grid" data-columns={1} className="flex min-h-0 min-w-0">
        {seat === undefined ? null : <SeatArea seat={seat} size={size} facing="far" pod={pod} />}
      </div>
    )
  }
  return (
    <div
      data-layout="grid"
      data-columns={plan.columns}
      className={cn('grid min-h-0 min-w-0 auto-rows-fr gap-2', COLUMNS[plan.columns])}
    >
      {plan.boards.map((seat) => (
        <SeatArea key={seat} seat={seat} size={size} facing="far" pod="line" />
      ))}
    </div>
  )
}
