import { memo, useRef } from 'react'
import type { CSSProperties } from 'react'
import { cn } from '@/lib/utils'
import type { PlayerId } from '@/protocol/generated'
import { Battlefield } from './Battlefield'
import type { CardSize } from './cardSize'
import { POD_COLUMN_PX } from './layout'
import type { PodPlace, SeatPlan } from './layout'
import { SeatPanel } from './SeatPanel'
import { SeatTile } from './SeatTile'
import { useEdgeFade } from './useEdgeFade'

interface SeatAreaProps {
  seat: PlayerId
  size: CardSize
  /** `near` is the viewer's own side, `far` an opponent's. */
  facing: 'near' | 'far'
  /** The pod beside the board as a column, above it as one line, or not drawn. */
  pod: PodPlace
  /** Draw the seat's hand in its pod; the viewer's goes in the hand dock instead. */
  hand?: boolean
}

/** The pod of a seat as a column beside its board; taller than the zone, it scrolls inside. */
function PodColumn({ seat, size, hand }: { seat: PlayerId; size: CardSize; hand: boolean }) {
  const column = useRef<HTMLDivElement>(null)
  useEdgeFade(column)
  return (
    <div
      ref={column}
      data-slot="pod-column"
      style={{ width: POD_COLUMN_PX } as CSSProperties}
      className="fade-scroll min-h-0 shrink-0"
    >
      <SeatPanel seat={seat} size={size} hand={hand} />
    </div>
  )
}

/** One seat: its pod and its battlefield, side by side or one over the other. */
export const SeatArea = memo(function SeatArea({ seat, size, facing, pod, hand = true }: SeatAreaProps) {
  const aside = pod === 'aside'
  return (
    <div
      data-slot="seat-area"
      data-seat-area={seat}
      className={cn('flex min-h-0 min-w-0 flex-1 gap-2', aside ? 'flex-row' : 'flex-col')}
    >
      {pod === 'aside' ? (
        <PodColumn seat={seat} size={size} hand={hand} />
      ) : pod === 'line' ? (
        <SeatPanel seat={seat} size={size} variant="line" hand={hand} />
      ) : null}
      <Battlefield seat={seat} size={size} facing={facing} />
    </div>
  )
})

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
        <ul
          role="list"
          aria-label="Opponents"
          data-slot="tile-strip"
          className="relative flex h-14 shrink-0 gap-2 overflow-x-auto p-1"
        >
          {plan.tiles.map((seat) => (
            <li key={seat} className="flex min-w-44 flex-1">
              <SeatTile seat={seat} focused={seat === plan.focused} onFocusSeat={onFocusSeat} />
            </li>
          ))}
        </ul>
        {plan.focused === null ? null : <SeatArea seat={plan.focused} size={size} facing="far" pod={pod} />}
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
