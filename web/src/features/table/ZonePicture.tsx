import { memo, useState } from 'react'
import type { CSSProperties } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { ObjectId } from '@/protocol/generated'
import { CardFace } from './CardFace'
import { CardWidthContext } from './ImageFace'
import { LEGAL_RING, LegalBadge, ZONE_ICON } from './PodParts'
import { emptyLabel, zoneLabel, zoneOpenLabel, zoneTitle } from './podFigures'
import { useLegalCount } from './prompt/marks'
import { usePreviewHandlers } from './usePreviewHandlers'
import { ZoneDialog } from './ZoneButton'
import type { ListedZone } from './ZoneButton'

interface ZonePictureProps {
  zone: ListedZone
  /** The seat the zone belongs to, in the title of the list. */
  owner: string
  /** The cards in the zone, oldest first; the last is on top. */
  ids: readonly ObjectId[]
  /** The picture's box in px. */
  width: number
  height: number
}

const NONE: readonly ObjectId[] = []

const BOX = 'relative grid h-(--card-h) w-(--card-w) shrink-0 place-items-center rounded-md'

const boxStyle = (width: number, height: number): CSSProperties =>
  ({ '--card-w': `${width}px`, '--card-h': `${height}px` }) as CSSProperties

/** An empty zone: a faint outline with the zone's icon and name, and no picture. */
function EmptyZone({ zone, width, height }: Omit<ZonePictureProps, 'owner' | 'ids'>) {
  const Icon = ZONE_ICON[zone]
  const label = zoneLabel(zone, 0)
  return (
    <button
      type="button"
      disabled
      aria-label={label}
      title={label}
      data-zone={zone}
      data-empty="true"
      style={boxStyle(width, height)}
      className={cn(
        BOX,
        'content-center justify-items-center gap-1 border border-dashed border-border-strong/60 text-muted-foreground',
      )}
    >
      <Icon aria-hidden="true" className="size-4" />
      <span aria-hidden="true" className="text-xs leading-4">
        {emptyLabel(zone, width)}
      </span>
    </button>
  )
}

/** The face of a zone's top card. It reads its own object and is memoised, so a batch redraws it only when that card changes. */
const TopFace = memo(function TopFace({ id, width }: { id: number; width: number }) {
  const object = usePlay((state) => state.objectsById[id])
  if (object === undefined) return null
  return (
    <CardWidthContext.Provider value={width}>
      <CardFace object={object} role={null} size="compact" fit imageSize="small" upright />
    </CardWidthContext.Provider>
  )
})

/** A zone with cards: the picture of the top one with the zone's icon and the count in its corner. */
function FilledZone({ zone, owner, ids, width, height }: ZonePictureProps) {
  const [open, setOpen] = useState(false)
  const topId = ids.at(-1) ?? 0
  const name = usePlay((state) => state.objectsById[topId]?.name)
  const handlers = usePreviewHandlers(topId, () => setOpen(true))
  const Icon = ZONE_ICON[zone]
  const legal = useLegalCount(zone === 'command' ? NONE : ids)
  return (
    <>
      <button
        type="button"
        aria-haspopup="dialog"
        aria-label={zoneOpenLabel(zone, ids.length, legal)}
        title={zoneTitle(zone, ids.length, legal, name)}
        data-zone={zone}
        data-legal={legal > 0 ? legal : undefined}
        data-top={topId}
        style={boxStyle(width, height)}
        {...handlers}
        className={cn(
          BOX,
          'outline-offset-2 outline-primary focus-visible:outline-2',
          name === undefined && 'bg-muted',
          legal > 0 && LEGAL_RING,
        )}
      >
        <TopFace id={topId} width={width} />
        <span
          aria-hidden="true"
          data-slot="zone-count"
          className="absolute right-0 bottom-0 inline-flex items-center gap-1 rounded-tl-md rounded-br-md bg-foreground px-1 text-xs leading-4 font-semibold text-background tabular-nums"
        >
          <Icon className="size-3" />
          {ids.length}
        </span>
        {legal > 0 ? <LegalBadge /> : null}
      </button>
      <ZoneDialog zone={zone} owner={owner} ids={ids} open={open} onOpenChange={setOpen} />
    </>
  )
}

/**
 * A zone drawn as the picture of its top card with the count in a corner. A press opens the list of every card and
 * hover or focus previews the top card. An empty zone is a faint outline with the zone's name and no picture. A token
 * or a card without a picture source draws its text face, as on the board.
 */
export function ZonePicture(props: ZonePictureProps) {
  return props.ids.length === 0 ? <EmptyZone {...props} /> : <FilledZone {...props} />
}
