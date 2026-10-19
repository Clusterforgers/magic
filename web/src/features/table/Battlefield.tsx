import { memo, useMemo, useRef } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import type { PlayerId } from '@/protocol/generated'
import type { RowKind } from './cardModel'
import type { CardSize } from './cardSize'
import { CardRow } from './CardRow'
import type { CardBox } from './CardRow'
import { LAND_SCALE } from './fit'
import type { FitGroup } from './fit'
import { useGhosts } from './fx/hooks'
import { useSeatNamer } from './names'
import { battlefieldLoad, battlefieldRow } from './selectors'
import type { BattlefieldLoad } from './selectors'
import { useEdgeFade } from './useEdgeFade'
import { stacksOf, useZoneFit } from './useZoneFit'

const ROW_NAME: Record<RowKind, string> = {
  lands: 'lands',
  creatures: 'creatures',
  other: 'other permanents',
}

// Creatures sit nearest the middle of the table: below an opponent's lands, above the viewer's.
const ORDER: Record<'near' | 'far', readonly RowKind[]> = {
  near: ['creatures', 'other', 'lands'],
  far: ['lands', 'other', 'creatures'],
}

const TAPPED: Record<RowKind, keyof BattlefieldLoad> = {
  creatures: 'creaturesTapped',
  other: 'otherTapped',
  lands: 'landsTapped',
}

/** Under this card width a face shows its name and power/toughness only; the preview has the rest. */
const FULL_FACE_MIN_PX = 76

interface PermanentRowProps {
  seat: PlayerId
  row: RowKind
  label: string
  /** The face to draw while the zone has no size to fit to. */
  size: CardSize
  /** The card width and height in px that the zone's fit gave this row; missing until the zone is measured. */
  width?: number | undefined
  height?: number | undefined
}

/** One row of a seat's permanents; it redraws only when the ids in it or its card size change. */
const PermanentRow = memo(function PermanentRow({
  seat,
  row,
  label,
  size,
  width,
  height,
}: PermanentRowProps) {
  const ids = usePlay(useShallow((state) => battlefieldRow(state, seat, row)))
  const ghosts = useGhosts(seat, row)
  const box = useMemo<CardBox | undefined>(
    () => (width === undefined || height === undefined ? undefined : { width, height }),
    [width, height],
  )
  const face = width === undefined ? size : width >= FULL_FACE_MIN_PX ? 'normal' : 'compact'
  return <CardRow ids={ids} ghosts={ghosts} label={label} size={face} box={box} />
})

interface BattlefieldProps {
  seat: PlayerId
  /** The size of creatures and other permanents; lands are always compact, since a land is a name and a tap. */
  size: CardSize
  /** `near` is the viewer's own side (creatures on top), `far` an opponent's (lands on top). */
  facing: 'near' | 'far'
}

/**
 * The permanents one seat controls, in rows of lands, creatures and everything else. The zone takes the room its
 * parent gives it and fits its cards to it (`useZoneFit`); what cannot fit even at the smallest card scrolls inside,
 * with the edge fade and a count of the permanents below.
 */
export const Battlefield = memo(function Battlefield({ seat, size, facing }: BattlefieldProps) {
  const owner = useSeatNamer()(seat)
  const load = usePlay(useShallow((state) => battlefieldLoad(state, seat)))
  const zone = useRef<HTMLDivElement>(null)
  useEdgeFade(zone)
  const order = ORDER[facing]
  const counts = order.map((row) => stacksOf(load[row], load[TAPPED[row]])).join(',')
  const groups = useMemo<FitGroup[]>(
    () =>
      counts.split(',').map((stacks, index) => ({
        stacks: Number(stacks),
        ...(order[index] === 'lands' ? { scale: LAND_SCALE } : {}),
      })),
    [counts, order],
  )
  const fit = useZoneFit(zone, groups)
  const empty = load.creatures + load.other + load.lands === 0
  return (
    <section
      aria-label={`${owner}'s battlefield`}
      data-battlefield-seat={seat}
      className="relative flex min-h-0 flex-col"
    >
      <div
        ref={zone}
        data-slot="battlefield-zone"
        className="fade-scroll grid min-h-0 flex-1 content-start gap-1 p-1"
      >
        {empty ? (
          <p className="px-1 py-3 text-sm text-muted-foreground">No permanents on the battlefield.</p>
        ) : null}
        {order.map((row, index) => (
          <PermanentRow
            key={row}
            seat={seat}
            row={row}
            label={`${owner}'s ${ROW_NAME[row]}`}
            size={row === 'lands' ? 'compact' : size}
            width={fit?.layout[index]?.width}
            height={fit?.layout[index]?.height}
          />
        ))}
      </div>
      {fit !== null && fit.scrolls && fit.hidden > 0 ? (
        <span
          aria-hidden="true"
          data-slot="more-hidden"
          className="pointer-events-none absolute right-2 bottom-1 rounded-full bg-foreground px-2 text-xs font-medium text-background tabular-nums"
        >
          +{fit.hidden}
        </span>
      ) : null}
    </section>
  )
})
