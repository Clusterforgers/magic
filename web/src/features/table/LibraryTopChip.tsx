/** The chip beside your library chip that shows the top card you may look at, and opens it to cast or play it. */
import { useMemo, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { ObjectId, PlayerView } from '@/protocol/generated'
import { CardRow } from './CardRow'
import { seenTop, topChipName, topDialogLine } from './libraryTop'
import { LEGAL_RING, LegalBadge, ZONE_ICON } from './PodParts'
import type { ChipDensity } from './PodParts'
import { usePreviewStore } from './preview'
import { useLegalCount } from './prompt/marks'
import { TopFace } from './TopFace'

/** The width of the top card's picture in the chip, in px. */
const THUMB_PX = 20
const THUMB_H = Math.round(THUMB_PX * 1.4)

interface LibraryTopChipProps {
  /** Your own seat; the chip draws nothing for another seat or while no top card is seen. */
  player: PlayerView
  density?: Exclude<ChipDensity, 'tile'>
}

/**
 * The top card of your library while you may look at it: its picture and the library's icon, with the ring and mark of
 * a legal card while a permission (Sigarda, Future Sight, Bolas's Citadel) lets you cast or play it now. The click opens
 * the card, and a click on it there casts or plays it, as a card of the graveyard does.
 */
export function LibraryTopChip({ player, density = 'line' }: LibraryTopChipProps) {
  const top = seenTop(player)
  const name = usePlay((state) => (top === undefined ? undefined : state.objectsById[top]?.name))
  const ids = useMemo(() => (top === undefined ? [] : [top]), [top])
  const legal = useLegalCount(ids) > 0
  // The card it was opened for: once that card leaves the top (cast or played), the dialog closes by itself.
  const [openFor, setOpenFor] = useState<ObjectId | null>(null)
  const preview = usePreviewStore()
  if (top === undefined || name === undefined) return null
  const Icon = ZONE_ICON.library
  const line = density === 'line'
  return (
    <>
      <Button
        variant="outline"
        size="xs"
        aria-haspopup="dialog"
        aria-label={topChipName(name, legal)}
        title={`Top of your library: ${name}`}
        data-zone="library-top"
        data-legal={legal ? 1 : undefined}
        onClick={() => setOpenFor(top)}
        className={cn(
          'h-auto min-w-0 py-px text-xs',
          line ? 'gap-0.5 px-0.5' : 'gap-1 px-1.5',
          legal && cn('relative', LEGAL_RING),
        )}
      >
        <span
          aria-hidden="true"
          className="pointer-events-none relative block shrink-0 overflow-hidden rounded-[3px]"
          style={{ width: THUMB_PX, height: THUMB_H }}
        >
          <TopFace id={top} width={THUMB_PX} />
        </span>
        <Icon aria-hidden="true" className={line ? 'size-3.5' : 'size-4'} />
        <span aria-hidden="true">Top</span>
        {legal ? <LegalBadge className="right-auto -left-1" /> : null}
      </Button>
      <Dialog
        open={openFor === top}
        onOpenChange={(next) => {
          setOpenFor(next ? top : null)
          if (!next) preview.dismiss()
        }}
      >
        <DialogContent>
          <DialogTitle>Top of your library</DialogTitle>
          <DialogDescription>{topDialogLine(legal)}</DialogDescription>
          <CardRow ids={ids} label="The top card of your library" size="normal" />
        </DialogContent>
      </Dialog>
    </>
  )
}
