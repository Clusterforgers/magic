import { memo, useCallback, useContext, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { usePlay } from '@/features/game/gameContext'
import type { ObjectId, PlayerId } from '@/protocol/generated'
import type { RowKind } from './cardModel'
import type { CardSize } from './cardSize'
import { CardRow } from './CardRow'
import type { CardBox } from './CardRow'
import { useGhosts } from './fx/hooks'
import { useSeatNamer } from './names'
import { battlefieldStacks, idsOfRows, keyOf, layoutSeat, sameEntries, visibleIds } from './rowStacks'
import type { RowEntry } from './rowStacks'
import { STACKS_BY_DEFAULT, TableOptionsContext } from './tableOptions'
import { useEdgeFade } from './useEdgeFade'
import { useMarkedIds } from './useMarkedIds'
import { useStable } from './useStable'
import { useContentBox } from './useZoneFit'

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

/** Under this card width a face shows its name and power/toughness only; the preview carries the rest. */
const FULL_FACE_MIN_PX = 76

const NONE: ReadonlySet<string> = new Set()

interface PermanentRowProps {
  seat: PlayerId
  row: RowKind
  label: string
  /** The face to draw while the zone has no size to fit to. */
  size: CardSize
  /** The stacks of the row as `layoutSeat` drew them. */
  entries: readonly RowEntry[]
  /** Opens or closes the stack that holds a member. */
  onToggle: (id: ObjectId) => void
  /** The card width and height in px that the zone's fit gave this row; missing until the zone is measured. */
  width?: number | undefined
  height?: number | undefined
}

/** One row of a seat's permanents; it redraws only when its stacks, their state or its card size change. */
const PermanentRow = memo(
  function PermanentRow({ seat, row, label, size, entries, onToggle, width, height }: PermanentRowProps) {
    const ghosts = useGhosts(seat, row)
    const ids = useMemo(() => visibleIds(entries), [entries])
    const box = useMemo<CardBox | undefined>(
      () => (width === undefined || height === undefined ? undefined : { width, height }),
      [width, height],
    )
    const face = width === undefined ? size : width >= FULL_FACE_MIN_PX ? 'normal' : 'compact'
    return (
      <CardRow
        ids={ids}
        stacks={entries}
        onToggleStack={onToggle}
        ghosts={ghosts}
        label={label}
        size={face}
        box={box}
      />
    )
  },
  // A tap that moves no permanent between stacks renames a stack but draws the same row.
  (a, b) =>
    a.seat === b.seat &&
    a.row === b.row &&
    a.label === b.label &&
    a.size === b.size &&
    a.onToggle === b.onToggle &&
    a.width === b.width &&
    a.height === b.height &&
    sameEntries(a.entries, b.entries),
)

interface BattlefieldProps {
  seat: PlayerId
  /** The size of creatures and other permanents; lands are always compact, since a land is a name and a tap. */
  size: CardSize
  /** `near` is the viewer's own side (creatures on top), `far` an opponent's (lands on top). */
  facing: 'near' | 'far'
}

/**
 * The permanents one seat controls, in rows of lands, creatures and everything else. Equal permanents share a box
 * with a "×N" chip (`group.ts`); a stack opens in place on a click or Enter, and a prompt that marks a member opens
 * it for as long as it needs each one. The zone takes the room its parent gives it and fits its boxes to it
 * (`layoutSeat`); what cannot fit even at the smallest card scrolls inside, with the edge fade and a count of the
 * boxes below.
 */
export const Battlefield = memo(function Battlefield({ seat, size, facing }: BattlefieldProps) {
  const owner = useSeatNamer()(seat)
  const grouped = useContext(TableOptionsContext).stacks ?? STACKS_BY_DEFAULT
  const stacks = usePlay(useStable((state) => battlefieldStacks(state, seat, grouped)))
  const zone = useRef<HTMLDivElement>(null)
  useEdgeFade(zone)
  const box = useContentBox(zone)
  const order = ORDER[facing]
  const permanents = useMemo(() => idsOfRows(stacks.plain), [stacks])
  const marked = useMarkedIds(permanents)
  const [opened, setOpened] = useState<ReadonlySet<string>>(NONE)
  const layout = useMemo(
    () => layoutSeat({ box, stacks, order, opened, marked }),
    [box, stacks, order, opened, marked],
  )
  // The toggle names a stack by a member, so a row that keeps its ids keeps the callback while keys change.
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
  return (
    <section
      aria-label={`${owner}'s battlefield`}
      data-battlefield-seat={seat}
      // Clips its zone, which cannot be shorter than its padding: a board squeezed to no room (a tall prompt sheet
      // under it) shows no sliver of a card.
      className="relative flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden"
    >
      <div
        ref={zone}
        data-slot="battlefield-zone"
        className="fade-scroll grid min-h-0 flex-1 content-start gap-1 p-1"
      >
        {permanents.length === 0 ? (
          <p className="px-1 py-3 text-sm text-muted-foreground">No permanents on the battlefield.</p>
        ) : null}
        {order.map((row, index) => (
          <PermanentRow
            key={row}
            seat={seat}
            row={row}
            label={`${owner}'s ${ROW_NAME[row]}`}
            size={row === 'lands' ? 'compact' : size}
            entries={layout.rows[row]}
            onToggle={toggle}
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
