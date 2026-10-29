import { memo, useCallback, useContext, useLayoutEffect, useMemo, useRef, useState } from 'react'
import type { CSSProperties } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { ObjectId, PlayerId } from '@/protocol/generated'
import type { RowKind } from './cardModel'
import type { CardSize } from './cardSize'
import { CardRow } from './CardRow'
import type { CardBox } from './CardRow'
import { useGhosts } from './fx/hooks'
import { SHAPES } from './laneFit'
import type { LanePlace } from './laneFit'
import { useSeatNamer } from './names'
import { battlefieldStacks, idsOfRows, keyOf, layoutSeat, sameEntries, visibleIds } from './rowStacks'
import type { RowEntry } from './rowStacks'
import { STACKS_BY_DEFAULT, TableOptionsContext } from './tableOptions'
import { useEdgeFade } from './useEdgeFade'
import { useMarkedIds } from './useMarkedIds'
import { useStable } from './useStable'
import { useContentBox } from './useZoneFit'

const LANE_NAME: Record<RowKind, string> = {
  lands: 'lands',
  creatures: 'creatures',
  other: 'other permanents',
}

/** Under this card width a face shows its name and power/toughness only; the preview carries the rest. */
const FULL_FACE_MIN_PX = 76
/** From this card width the text face takes the larger type of the `medium` size. */
const MEDIUM_FACE_MIN_PX = 150

const NONE: ReadonlySet<string> = new Set()

const faceOf = (width: number | undefined, size: CardSize): CardSize => {
  if (width === undefined) return size
  if (width >= MEDIUM_FACE_MIN_PX) return 'medium'
  return width >= FULL_FACE_MIN_PX ? 'normal' : 'compact'
}

interface LaneProps {
  seat: PlayerId
  lane: RowKind
  label: string
  /** The face to draw while the zone has no size to fit to. */
  size: CardSize
  /** The stacks of the lane as `layoutSeat` drew them. */
  entries: readonly RowEntry[]
  /** Opens or closes the stack that holds a member. */
  onToggle: (id: ObjectId) => void
  /** The card box and the lane's width in px from the zone's fit; missing until the zone is measured. */
  width?: number | undefined
  height?: number | undefined
  span?: number | undefined
}

/** One lane of a seat's permanents; it redraws only when its stacks, their state or its sizes change. */
const Lane = memo(
  function Lane({ seat, lane, label, size, entries, onToggle, width, height, span }: LaneProps) {
    const ghosts = useGhosts(seat, lane)
    const ids = useMemo(() => visibleIds(entries), [entries])
    const box = useMemo<CardBox | undefined>(
      () => (width === undefined || height === undefined ? undefined : { width, height }),
      [width, height],
    )
    if (ids.length === 0 && ghosts.length === 0) return null
    return (
      <div
        data-lane={lane}
        style={span === undefined ? undefined : ({ width: span } as CSSProperties)}
        // The tint lies around the lane as an outline, so it takes none of the room the fit counts.
        className={cn('shrink-0 rounded-md bg-lane outline-4 outline-lane', lane === 'other' && 'ml-auto')}
      >
        <CardRow
          ids={ids}
          stacks={entries}
          onToggleStack={onToggle}
          ghosts={ghosts}
          label={label}
          size={faceOf(width, lane === 'lands' ? 'compact' : size)}
          box={box}
        />
      </div>
    )
  },
  // A tap that moves no permanent between stacks renames a stack but draws the same lane.
  (a, b) =>
    a.seat === b.seat &&
    a.lane === b.lane &&
    a.label === b.label &&
    a.size === b.size &&
    a.onToggle === b.onToggle &&
    a.width === b.width &&
    a.height === b.height &&
    a.span === b.span &&
    sameEntries(a.entries, b.entries),
)

interface BattlefieldProps {
  seat: PlayerId
  /** The face of the cards while the zone has no size to fit to. */
  size: CardSize
  /** `near` is the viewer's own side, `far` an opponent's: the bands mirror each other top to bottom. */
  facing: 'near' | 'far'
}

/**
 * The permanents one seat controls, in lanes of lands, creatures and other permanents (`laneFit.ts`): creatures
 * nearest the middle of the table, lands at the left and other permanents at the right, the same on every board
 * and mirrored top to bottom for an opponent. An empty lane takes no room. Equal permanents share a box with a "×N"
 * chip (`group.ts`); a stack opens in place on a click or Enter, and a prompt that marks a member opens it for as long
 * as it needs each one. The zone fits the largest card at which every lane fits its box (`layoutSeat`); what cannot
 * fit even at the smallest card scrolls inside, with the edge fade and a count of the boxes below.
 */
export const Battlefield = memo(function Battlefield({ seat, size, facing }: BattlefieldProps) {
  const owner = useSeatNamer()(seat)
  const grouped = useContext(TableOptionsContext).stacks ?? STACKS_BY_DEFAULT
  const stacks = usePlay(useStable((state) => battlefieldStacks(state, seat, grouped)))
  const zone = useRef<HTMLDivElement>(null)
  useEdgeFade(zone)
  const box = useContentBox(zone)
  const permanents = useMemo(() => idsOfRows(stacks.plain), [stacks])
  const marked = useMarkedIds(permanents)
  const [opened, setOpened] = useState<ReadonlySet<string>>(NONE)
  const layout = useMemo(
    () => layoutSeat({ box, stacks, facing, opened, marked }),
    [box, stacks, facing, opened, marked],
  )
  // The toggle names a stack by a member, so a lane that keeps its ids keeps the callback while keys change.
  const keys = useRef(layout.stacks)
  useLayoutEffect(() => {
    keys.current = layout.stacks
  })
  const toggle = useCallback((id: ObjectId) => {
    const key = keyOf(keys.current, id)
    if (key === undefined) return
    setOpened((was) => {
      const next = new Set(was)
      if (!next.delete(key)) next.add(key)
      return next
    })
  }, [])
  const { fit } = layout
  // Before the zone is measured every lane is drawn in the two-band shape at its own size.
  const bands = useMemo<readonly (readonly (LanePlace | RowKind)[])[]>(() => {
    if (fit !== null) return fit.bands.map((band) => band.lanes)
    const outwards = SHAPES.two
    return facing === 'near' ? outwards : [...outwards].reverse()
  }, [fit, facing])
  return (
    <section
      aria-label={`${owner}'s battlefield`}
      data-battlefield-seat={seat}
      data-board-shape={fit?.shape}
      // Clips its zone, which cannot be shorter than its padding: a board squeezed to no room (a tall prompt sheet
      // under it) shows no sliver of a card.
      className="relative flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden"
    >
      <div
        ref={zone}
        data-slot="battlefield-zone"
        className={cn(
          'fade-scroll flex min-h-0 flex-1 flex-col gap-3 p-1',
          // An opponent's lanes end at the middle of the table, as yours start there.
          facing === 'far' && fit !== null && !fit.scrolls && 'justify-end',
        )}
      >
        {permanents.length === 0 ? (
          <p className="px-1 py-3 text-sm text-muted-foreground">No permanents on the battlefield.</p>
        ) : null}
        {bands.map((band) => {
          const lanes = band.map((lane) => (typeof lane === 'string' ? { lane } : lane))
          return (
            <div
              key={lanes.map((lane) => lane.lane).join('-')}
              data-slot="board-band"
              className={cn(
                'flex shrink-0 items-start gap-4',
                lanes.length === 1 && lanes[0]?.lane === 'creatures' ? 'justify-center' : 'justify-between',
              )}
            >
              {lanes.map((place) => (
                <Lane
                  key={place.lane}
                  seat={seat}
                  lane={place.lane}
                  label={`${owner}'s ${LANE_NAME[place.lane]}`}
                  size={size}
                  entries={layout.rows[place.lane]}
                  onToggle={toggle}
                  width={'width' in place ? place.width : undefined}
                  height={'height' in place ? place.height : undefined}
                  span={'span' in place ? place.span : undefined}
                />
              ))}
            </div>
          )
        })}
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
