/** The chip beside a seat's hand and library chips that lists the cards it showed to every seat, face up. */
import { Eye } from 'lucide-react'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { cn } from '@/lib/utils'
import type { PlayerView } from '@/protocol/generated'
import { CardRow } from './CardRow'
import { DENSITY } from './PodParts'
import type { ChipDensity } from './PodParts'
import { usePreviewStore } from './preview'
import { revealedChipName, revealedLabel, revealedOf, revealedWhere } from './revealed'
import type { RevealedCards } from './revealed'

interface RevealedChipProps {
  /** The seat's name, in the title of the list. */
  owner: string
  /** The seat; its revealed hand cards and top card are what the chip lists. */
  player: PlayerView
}

/** The cards of a seat that are face up to every seat, in a dialog. */
function RevealedDialog({
  owner,
  cards,
  open,
  onOpenChange,
}: {
  owner: string
  cards: RevealedCards
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const preview = usePreviewStore()
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
        <DialogTitle>{owner}&rsquo;s revealed cards</DialogTitle>
        <DialogDescription>{revealedWhere(cards)}.</DialogDescription>
        <CardRow ids={cards.all} label={`${owner}'s revealed cards`} size="normal" />
      </DialogContent>
    </Dialog>
  )
}

/**
 * An eye with the number of cards a seat showed (its revealed hand cards and top card), that opens the list with the
 * cards face up. Nothing is drawn while the seat shows none.
 */
export function RevealedChip({
  owner,
  player,
  density = 'panel',
}: RevealedChipProps & { density?: Exclude<ChipDensity, 'tile'> }) {
  const [open, setOpen] = useState(false)
  const cards = revealedOf(player)
  if (cards.all.length === 0) return null
  const label = revealedChipName(cards)
  const line = density === 'line'
  return (
    <>
      <Button
        variant="outline"
        size="xs"
        aria-haspopup="dialog"
        aria-label={`${label}, opens the list`}
        title={label}
        data-zone="revealed"
        onClick={() => setOpen(true)}
        className={cn('min-w-0 text-xs', line ? 'gap-0.5 px-0.5' : 'gap-1 px-1.5')}
      >
        <Eye aria-hidden="true" className={line ? 'size-3.5' : 'size-4'} />
        <span aria-hidden="true" className="tabular-nums">
          {cards.all.length}
        </span>
      </Button>
      <RevealedDialog owner={owner} cards={cards} open={open} onOpenChange={setOpen} />
    </>
  )
}

/** The figure of a tile, which has no button of its own inside: an eye and the count, named in words. */
export function RevealedFigure({ player }: { player: PlayerView }) {
  const cards = revealedOf(player)
  if (cards.all.length === 0) return null
  const label = revealedLabel(cards.all.length)
  return (
    <span
      role="img"
      aria-label={label}
      title={label}
      data-zone="revealed"
      className={cn(DENSITY.tile.chip, 'text-muted-foreground')}
    >
      <Eye aria-hidden="true" className={cn('shrink-0', DENSITY.tile.icon)} />
      <span aria-hidden="true" className="tabular-nums">
        {cards.all.length}
      </span>
    </span>
  )
}
