import { BookOpenIcon } from 'lucide-react'
import { cn } from '@/lib/utils'
import { cardsToRead, readPath } from '@/lib/toRead'
import type { ReadTarget } from '@/lib/toRead'

interface ToReadProps {
  /** Distinct cards of the deck whose draft waits for a read (`summary.to_read`). */
  count: number
  /** The deck whose cards are read; none for a deck that is not saved. */
  target?: ReadTarget
  className?: string
}

/**
 * "3 cards to read" with a book beside it, where a deck check says drafts wait for a player; nothing for none. It is
 * plain text until the review screen exists: `data-read-path` holds the address (`/read?deck=<id>` or
 * `/read?precon=<code>`) the words will link to.
 */
export function ToRead({ count, target, className }: ToReadProps) {
  if (count <= 0) return null
  return (
    <span
      data-slot="to-read"
      data-read-path={target === undefined ? undefined : readPath(target)}
      className={cn('inline-flex items-center gap-1 font-medium text-foreground', className)}
    >
      <BookOpenIcon aria-hidden="true" className="size-4 shrink-0 text-support-read" />
      {cardsToRead(count)}
    </span>
  )
}
