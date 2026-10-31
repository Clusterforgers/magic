import { BookOpenIcon } from 'lucide-react'
import { cn } from '@/lib/utils'
import { DRAFT_NOTE } from '@/lib/toRead'
import type { ReadState } from '@/protocol/generated'

interface DraftNoteProps {
  /** The read state of the card's draft, as the deck report gives it. */
  draft: ReadState | undefined
  className?: string
}

/**
 * The line under a card played by hand whose draft is waiting for a read or was found wrong; nothing for a card with no
 * draft or an approved one (its state already says "Read").
 */
export function DraftNote({ draft, className }: DraftNoteProps) {
  if (draft !== 'pending' && draft !== 'wrong') return null
  return (
    <p
      data-slot="draft-note"
      data-draft={draft}
      className={cn('flex items-start gap-1 text-xs text-muted-foreground', className)}
    >
      <BookOpenIcon aria-hidden="true" className="mt-px size-4 shrink-0 text-support-read" />
      {DRAFT_NOTE[draft]}
    </p>
  )
}
