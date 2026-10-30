import { useMemo } from 'react'
import type { CSSProperties } from 'react'
import { cn } from '@/lib/utils'
import type { ObjectId } from '@/protocol/generated'
import { CardRow } from './CardRow'
import { TaxChips } from './CommanderChips'
import type { CommanderChip } from './CommanderChips'
import { CORNER_GAP_PX, CORNER_ROOM_PX, PARTNER_HEAD_PX, planCorner } from './cornerFit'
import { ZoneButton } from './ZoneButton'
import { ZonePicture } from './ZonePicture'
import { ZONE_ICON } from './PodParts'
import { emptyLabel, zoneLabel } from './podFigures'
import { NO_IDS } from './selectors'

interface SeatCornerProps {
  /** The seat's name, in the titles and names of the lists. */
  owner: string
  /** The cards of the graveyard, oldest first. */
  graveyard: readonly ObjectId[]
  /** The cards in exile that the seat owns, oldest first. */
  exile: readonly ObjectId[]
  /** The cards of the command zone, oldest first. */
  commanders: readonly ObjectId[]
  /** The viewer's own commanders wait in the hand dock, so the corner draws no card of them. */
  inDock: boolean
  /** Draws the command zone's outline while it holds no card: the seat plays Commander. */
  commandSlot: boolean
  /** The tax chips of the commanders the corner draws. */
  tax: readonly CommanderChip[]
  /** The room under the pod's other blocks in px, and the lane card width beside it; null while they are not measured. */
  height: number | null
  card: number | null
}

/** The command zone with nothing in it: a faint outline, as the other empty zones. */
function EmptyCommand({ width, height }: { width: number; height: number }) {
  const Icon = ZONE_ICON.command
  const label = zoneLabel('command', 0)
  return (
    <span
      role="img"
      aria-label={label}
      title={label}
      data-zone="command"
      data-empty="true"
      style={{ width, height }}
      className="grid shrink-0 content-center justify-items-center gap-1 rounded-md border border-dashed border-border-strong/60 text-muted-foreground"
    >
      <Icon aria-hidden="true" className="size-4" />
      <span aria-hidden="true" className="text-[0.6875rem] leading-4">
        {emptyLabel('command', width)}
      </span>
    </span>
  )
}

interface CommanderSlotProps {
  owner: string
  ids: readonly ObjectId[]
  width: number
  height: number
  tax: readonly CommanderChip[]
}

/** The commanders as cards, partners lying over each other with the head of the first showing, the tax on their foot. */
function CommanderSlot({ owner, ids, width, height, tax }: CommanderSlotProps) {
  const fan = ids.length > 1 ? { pile: ids.length, head: PARTNER_HEAD_PX } : undefined
  return (
    <div data-slot="corner-commander" data-zone="command" className="relative shrink-0">
      <CardRow
        ids={ids}
        label={`${owner}'s command zone`}
        size="compact"
        box={{ width, height }}
        fan={fan}
        className="content-start"
      />
      {tax.length === 0 ? null : (
        <div className="pointer-events-none absolute inset-x-0 bottom-0 flex flex-wrap justify-center gap-px">
          <TaxChips chips={tax} density="tile" compact className="bg-foreground text-background" />
        </div>
      )}
    </div>
  )
}

/**
 * The corner of a seat's pod: its commanders, graveyard and exile as pictures in a grid sized by `planCorner`, each with
 * its count, the commanders with their tax. Where the pod column leaves no room for the least picture it keeps the
 * zone chips alone, in one row, and the tax chips under them.
 */
export function SeatCorner({
  owner,
  graveyard,
  exile,
  commanders,
  inDock,
  commandSlot,
  tax,
  height,
  card,
}: SeatCornerProps) {
  const drawn = inDock ? NO_IDS : commanders
  const slots = (drawn.length > 0 || (!inDock && commandSlot) ? 1 : 0) + 2
  const extra = Math.max(0, drawn.length - 1) * PARTNER_HEAD_PX
  const plan = useMemo(
    () => planCorner({ slots, width: CORNER_ROOM_PX, height, card, extra }),
    [slots, height, card, extra],
  )
  const group = `${owner}'s zones`
  if (plan === null) {
    // No room for the least picture is none for a small card either (it is as high), so the chips come whole.
    return (
      <>
        <div
          role="group"
          aria-label={group}
          data-slot="corner-chips"
          className={cn('grid min-w-0 gap-1', commanders.length > 0 ? 'grid-cols-3' : 'grid-cols-2')}
        >
          <ZoneButton zone="graveyard" owner={owner} ids={graveyard} />
          <ZoneButton zone="exile" owner={owner} ids={exile} />
          {commanders.length > 0 ? <ZoneButton zone="command" owner={owner} ids={commanders} /> : null}
        </div>
        {tax.length === 0 ? null : (
          <div data-slot="commander-chips" className="flex flex-wrap gap-1">
            <TaxChips chips={tax} density="panel" className="max-w-full" />
          </div>
        )}
      </>
    )
  }
  const style = {
    gridTemplateColumns: `repeat(${plan.columns}, ${plan.width}px)`,
    gap: CORNER_GAP_PX,
  } as CSSProperties
  return (
    <div
      role="group"
      aria-label={group}
      data-slot="seat-corner"
      data-picture={plan.width}
      style={style}
      className="grid content-start items-start justify-center"
    >
      {drawn.length > 0 ? (
        <CommanderSlot owner={owner} ids={drawn} width={plan.width} height={plan.height} tax={tax} />
      ) : !inDock && commandSlot ? (
        <EmptyCommand width={plan.width} height={plan.height} />
      ) : null}
      <ZonePicture zone="graveyard" owner={owner} ids={graveyard} width={plan.width} height={plan.height} />
      <ZonePicture zone="exile" owner={owner} ids={exile} width={plan.width} height={plan.height} />
    </div>
  )
}
