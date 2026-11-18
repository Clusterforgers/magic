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
import { BoardFloorContext } from './boardFloor'
import { GridCardContext } from './gridCard'
import { LandFoldContext } from './landFold'
import { promptKey } from './boardZoom'
import { LANE_LIMITS, SHAPES } from './laneFit'
import type { LanePlace } from './laneFit'
import { heldBands, holdBands } from './holdFit'
import type { BandHold, FitHold } from './holdFit'
import { useSeatNamer } from './names'
import {
  battlefieldStacks,
  foldLands,
  idsOfRows,
  keyOf,
  layoutSeat,
  sameEntries,
  visibleIds,
} from './rowStacks'
import type { RowEntry } from './rowStacks'
import { STACKS_BY_DEFAULT, TableOptionsContext } from './tableOptions'
import { useEdgeFade } from './useEdgeFade'
import { ZONE_PAD_PX } from './boardSplit'
import { usePreviewInset } from './previewReserve'
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

/** A band as drawn: its lanes (placed once the zone is measured) and the least height it keeps in px (0: none). */
interface DrawnBand {
  key: string
  lanes: readonly (LanePlace | { lane: RowKind })[]
  height: number
}

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
  /** The fanned piles of a lands lane (`LanePlace.pile` and `head`); missing or 1 for a plain lane. */
  pile?: number | undefined
  head?: number | undefined
  /** The tuck of the lane (`LanePlace.tuck` and `lift`); missing or 0 without attachments tucked at a host. */
  tuck?: number | undefined
  lift?: number | undefined
}

/** One lane of a seat's permanents; it redraws only when its stacks, their state or its sizes change. */
const Lane = memo(
  function Lane({
    seat,
    lane,
    label,
    size,
    entries,
    onToggle,
    width,
    height,
    span,
    pile,
    head,
    tuck,
    lift,
  }: LaneProps) {
    const ghosts = useGhosts(seat, lane)
    const ids = useMemo(() => visibleIds(entries), [entries])
    const box = useMemo<CardBox | undefined>(
      () => (width === undefined || height === undefined ? undefined : { width, height }),
      [width, height],
    )
    const fan = useMemo(
      () => (pile === undefined || head === undefined || pile < 2 ? undefined : { pile, head }),
      [pile, head],
    )
    const tucks = useMemo(
      () => (tuck === undefined || lift === undefined || lift <= 0 ? undefined : { each: tuck, lift }),
      [tuck, lift],
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
          fan={fan}
          tuck={tucks}
          fromRight={lane === 'other'}
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
    a.pile === b.pile &&
    a.head === b.head &&
    a.tuck === b.tuck &&
    a.lift === b.lift &&
    sameEntries(a.entries, b.entries),
)

interface BattlefieldProps {
  seat: PlayerId
  /** The face of the cards while the zone has no size to fit to. */
  size: CardSize
  /** `near` is the viewer's own side, `far` an opponent's: the bands mirror each other top to bottom. */
  facing: 'near' | 'far'
  /** Told the creatures' card width of the fit in px (null before the zone is measured), which the seat's corner follows. */
  onCardWidth?: (width: number | null) => void
}

/**
 * The permanents one seat controls, in lanes of lands, creatures and other permanents (`laneFit.ts`): creatures
 * nearest the middle of the table, lands at the left and other permanents at the right, the same on every board
 * and mirrored top to bottom for an opponent. An empty lane takes no room. Equal permanents share a box with a "×N"
 * chip (`group.ts`); a stack opens in place on a click or Enter, and a prompt that marks a member opens it for as long
 * as it needs each one. The zone fits the largest card at which every lane fits its box (`layoutSeat`); what cannot
 * fit even at the smallest card scrolls inside, with the edge fade and a count of the boxes below.
 */
export const Battlefield = memo(function Battlefield({ seat, size, facing, onCardWidth }: BattlefieldProps) {
  const owner = useSeatNamer()(seat)
  const grouped = useContext(TableOptionsContext).stacks ?? STACKS_BY_DEFAULT
  const stacks = usePlay(useStable((state) => battlefieldStacks(state, seat, grouped)))
  const zone = useRef<HTMLDivElement>(null)
  useEdgeFade(zone)
  const inset = usePreviewInset(zone)
  const box = useContentBox(zone)
  const permanents = useMemo(() => idsOfRows(stacks.plain), [stacks])
  const marked = useMarkedIds(permanents)
  const [opened, setOpened] = useState<ReadonlySet<string>>(NONE)
  // An opponent's lands fold into its pod's lands chip, so its creatures and other permanents get the board's room; the
  // chip lays them out, and a prompt that marks one of them lays them out while it waits.
  const fold = useContext(LandFoldContext)
  const landIds = useMemo(() => stacks.plain.lands.flatMap((stack) => stack.ids), [stacks])
  // Priority marks every land that taps for mana; only a prompt beyond it (a target, a sacrifice) lays the lands out.
  const asking = usePlay(promptKey) !== null
  const landMarked = asking && landIds.some((id) => marked.has(id))
  const folded = fold !== null && !fold.open && landIds.length > 0 && !landMarked
  const shown = useMemo(() => (folded ? foldLands(stacks) : stacks), [folded, stacks])
  const creatureGhosts = useGhosts(seat, 'creatures').length
  const otherGhosts = useGhosts(seat, 'other').length
  const landGhostCount = useGhosts(seat, 'lands').length
  const landGhosts = folded ? 0 : landGhostCount
  const grid = useContext(GridCardContext)
  const most = grid?.most ?? null
  const least = useContext(BoardFloorContext)
  // The card width and shape drawn last: the next fit keeps them while they fit (`fitHeld`), so one permanent more or
  // less does not resize the board. Read when the inputs change, written once the fit is drawn.
  const hold = useRef<FitHold | null>(null)
  // The fit of the box alone, which a grid of boards shares out as one size; it never depends on that size.
  const alone = useMemo(
    () =>
      layoutSeat({
        box,
        stacks: shown,
        facing,
        opened,
        marked,
        held: { creatures: creatureGhosts, other: otherGhosts, lands: landGhosts },
        least,
        hold: hold.current,
      }),
    [box, shown, facing, opened, marked, creatureGhosts, otherGhosts, landGhosts, least],
  )
  const aloneWidth = alone.fit?.cardWidth ?? null
  const layout = useMemo(
    () =>
      most === null || aloneWidth === null || aloneWidth <= most
        ? alone
        : layoutSeat({
            box,
            stacks: shown,
            facing,
            opened,
            marked,
            held: { creatures: creatureGhosts, other: otherGhosts, lands: landGhosts },
            most,
            least,
            hold: hold.current,
          }),
    [
      alone,
      aloneWidth,
      most,
      box,
      shown,
      facing,
      opened,
      marked,
      creatureGhosts,
      otherGhosts,
      landGhosts,
      least,
    ],
  )
  const report = grid?.report
  useLayoutEffect(() => {
    report?.(seat, aloneWidth)
  }, [report, seat, aloneWidth])
  useLayoutEffect(() => () => report?.(seat, null), [report, seat])
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
  const cardWidth = fit?.cardWidth ?? null
  useLayoutEffect(() => {
    hold.current = fit === null || fit.scrolls ? null : { width: fit.cardWidth, shape: fit.shape }
  }, [fit])
  useLayoutEffect(() => {
    onCardWidth?.(cardWidth)
  }, [onCardWidth, cardWidth])
  // The bands' heights since the last size step: a band keeps its height until the card size or shape changes, so a
  // line that empties or a pile that shortens moves no lane beyond it (an opponent's board stands from the middle out).
  const bandHold = useRef<BandHold | null>(null)
  const boxHeight = box?.height ?? 0
  const kept = useMemo(
    () => holdBands(bandHold.current, fit, boxHeight, LANE_LIMITS.bandGap),
    [fit, boxHeight],
  )
  useLayoutEffect(() => {
    bandHold.current = kept
  }, [kept])
  // Before the zone is measured every lane is drawn in the two-band shape at its own size.
  const bands = useMemo<readonly DrawnBand[]>(() => {
    if (fit !== null) return heldBands(fit, kept, facing)
    const outwards = SHAPES.two
    const drawn = facing === 'near' ? outwards : [...outwards].reverse()
    return drawn.map((lanes) => ({ key: lanes.join('-'), lanes: lanes.map((lane) => ({ lane })), height: 0 }))
  }, [fit, kept, facing])
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
        data-preview-inset={inset.left + inset.right > 0 ? inset.left + inset.right : undefined}
        // A pinned preview over the board narrows it, so the lanes refit beside the panel and no card lies under it.
        style={
          inset.left + inset.right > 0
            ? { paddingLeft: ZONE_PAD_PX + inset.left, paddingRight: ZONE_PAD_PX + inset.right }
            : undefined
        }
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
          return (
            <div
              key={band.key}
              data-slot="board-band"
              data-band={band.key}
              style={band.height > 0 ? { minHeight: band.height } : undefined}
              // Lanes start at the board's left edge, so the room a short row leaves stays at the right, where a
              // side preview stands.
              className="flex shrink-0 items-start justify-start gap-3"
            >
              {band.lanes.map((place) => (
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
                  pile={'pile' in place ? place.pile : undefined}
                  head={'head' in place ? place.head : undefined}
                  tuck={'tuck' in place ? place.tuck : undefined}
                  lift={'lift' in place ? place.lift : undefined}
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
