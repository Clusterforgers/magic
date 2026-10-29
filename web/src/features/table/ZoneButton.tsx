import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { cn } from '@/lib/utils'
import type { ObjectId } from '@/protocol/generated'
import { CardRow } from './CardRow'
import { ZONE_ICON } from './PodParts'
import type { ChipDensity } from './PodParts'
import { cardCount, zoneLabel, ZONE_NAME } from './podFigures'
import type { ZoneKey } from './podFigures'
import { usePreviewStore } from './preview'

/** The zones whose cards a pod lists in a dialog. */
export type ListedZone = Extract<ZoneKey, 'graveyard' | 'exile' | 'command'>

interface ZoneDialogProps {
  zone: ListedZone
  /** The seat the zone belongs to, in the title of the list. */
  owner: string
  /** The cards in the zone, oldest first. */
  ids: readonly ObjectId[]
  open: boolean
  onOpenChange: (open: boolean) => void
}

/** The list of every card of a zone, most recent first, in a dialog. */
export function ZoneDialog({ zone, owner, ids, open, onOpenChange }: ZoneDialogProps) {
  const preview = usePreviewStore()
  const name = ZONE_NAME[zone].toLowerCase()
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        onOpenChange(next)
        // A card previewed from inside the list would otherwise stay up behind the closed dialog.
        if (!next) preview.dismiss()
      }}
    >
      <DialogContent>
        <DialogTitle>
          {owner}&rsquo;s {name}
        </DialogTitle>
        <DialogDescription>{cardCount(ids.length)}, most recent first.</DialogDescription>
        <CardRow ids={[...ids].reverse()} label={`${owner}'s ${name} cards`} size="normal" />
      </DialogContent>
    </Dialog>
  )
}

interface ZoneButtonProps {
  /** The zone whose cards the button lists. */
  zone: ListedZone
  /** The seat the zone belongs to, in the title of the list. */
  owner: string
  /** The cards in the zone, oldest first. */
  ids: readonly ObjectId[]
  /** The panel's chip, or the smaller one of a one-line pod. */
  density?: Exclude<ChipDensity, 'tile'>
}

/** A chip with the zone's icon and count that opens the list of every card in it, most recent first. */
export function ZoneButton({ zone, owner, ids, density = 'panel' }: ZoneButtonProps) {
  const [open, setOpen] = useState(false)
  const Icon = ZONE_ICON[zone]
  const label = zoneLabel(zone, ids.length)
  const line = density === 'line'
  return (
    <>
      <Button
        variant="outline"
        size="xs"
        disabled={ids.length === 0}
        aria-haspopup="dialog"
        aria-label={ids.length === 0 ? label : `${label}, opens the list`}
        title={label}
        data-zone={zone}
        onClick={() => setOpen(true)}
        className={cn('min-w-0 text-xs', line ? 'gap-0.5 px-0.5' : 'gap-1 px-1.5')}
      >
        <Icon aria-hidden="true" className={line ? 'size-3.5' : 'size-4'} />
        <span aria-hidden="true" className="tabular-nums">
          {ids.length}
        </span>
      </Button>
      <ZoneDialog zone={zone} owner={owner} ids={ids} open={open} onOpenChange={setOpen} />
    </>
  )
}
