import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import type { ObjectId } from '@/protocol/generated'
import { CardRow } from './CardRow'

interface ZoneButtonProps {
  /** "Graveyard" or "Exile". */
  zone: string
  /** The seat the zone belongs to, in the title of the list. */
  owner: string
  /** The cards in the zone, oldest first. */
  ids: readonly ObjectId[]
}

/** A button that opens the list of every card in a zone, most recent first. */
export function ZoneButton({ zone, owner, ids }: ZoneButtonProps) {
  const [open, setOpen] = useState(false)
  const noun = ids.length === 1 ? 'card' : 'cards'
  return (
    <>
      <Button
        variant="outline"
        size="sm"
        disabled={ids.length === 0}
        aria-haspopup="dialog"
        onClick={() => setOpen(true)}
      >
        {/* Explicit spaces: a screen reader joins the pieces of a name, and an inline span adds none. */}
        {zone} <span className="tabular-nums">{ids.length}</span>
        <span className="sr-only">
          {' '}
          {noun}
          {ids.length === 0 ? '' : ', opens the list'}
        </span>
      </Button>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent>
          <DialogTitle>
            {owner}&rsquo;s {zone.toLowerCase()}
          </DialogTitle>
          <DialogDescription>
            {ids.length} {noun}, most recent first.
          </DialogDescription>
          <CardRow ids={[...ids].reverse()} label={`${owner}'s ${zone.toLowerCase()} cards`} size="normal" />
        </DialogContent>
      </Dialog>
    </>
  )
}
