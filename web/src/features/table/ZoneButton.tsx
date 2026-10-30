import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { cn } from '@/lib/utils'
import type { ObjectId } from '@/protocol/generated'
import { CardRow } from './CardRow'
import { LEGAL_RING, LegalBadge, ZONE_ICON } from './PodParts'
import type { ChipDensity } from './PodParts'
import { cardCount, legalWords, zoneLabel, zoneOpenLabel, zoneTitle, ZONE_NAME } from './podFigures'
import type { ZoneKey } from './podFigures'
import { usePreviewStore } from './preview'
import { useLegalCount } from './prompt/marks'

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

/** The sentence "2 can be played or used." under the list's title; it reads the marks only while the list is open. */
function LegalLine({ ids }: { ids: readonly ObjectId[] }) {
  const legal = useLegalCount(ids)
  return legal === 0 ? null : <> {legalWords(legal)}.</>
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
        <DialogDescription>
          {cardCount(ids.length)}, most recent first.
          <LegalLine ids={ids} />
        </DialogDescription>
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
  /** More classes for the button, to size it in its row. */
  className?: string
}

const NONE: readonly ObjectId[] = []

/**
 * A chip with the zone's icon and count that opens the list of every card in it, most recent first. A graveyard or
 * exile with a card the viewer can play or use now (flashback, an ability) wears the ring of a legal card and a play
 * mark, and says how many in its name; the command zone does not, its commanders are cast from the hand dock.
 */
export function ZoneButton({ zone, owner, ids, density = 'panel', className }: ZoneButtonProps) {
  const [open, setOpen] = useState(false)
  const Icon = ZONE_ICON[zone]
  const legal = useLegalCount(zone === 'command' ? NONE : ids)
  const line = density === 'line'
  return (
    <>
      <Button
        variant="outline"
        size="xs"
        disabled={ids.length === 0}
        aria-haspopup="dialog"
        aria-label={ids.length === 0 ? zoneLabel(zone, 0) : zoneOpenLabel(zone, ids.length, legal)}
        title={zoneTitle(zone, ids.length, legal)}
        data-zone={zone}
        data-legal={legal > 0 ? legal : undefined}
        onClick={() => setOpen(true)}
        className={cn(
          'min-w-0 text-xs',
          line ? 'gap-0.5 px-0.5' : 'gap-1 px-1.5',
          legal > 0 && cn('relative', LEGAL_RING),
          className,
        )}
      >
        <Icon aria-hidden="true" className={line ? 'size-3.5' : 'size-4'} />
        <span aria-hidden="true" className="tabular-nums">
          {ids.length}
        </span>
        {legal > 0 ? <LegalBadge className="right-auto -left-1" /> : null}
      </Button>
      <ZoneDialog zone={zone} owner={owner} ids={ids} open={open} onOpenChange={setOpen} />
    </>
  )
}
