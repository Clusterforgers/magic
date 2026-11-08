import { Link } from '@tanstack/react-router'
import { BookOpenIcon } from 'lucide-react'
import { cn } from '@/lib/utils'
import { cardsToRead, readPath } from '@/lib/toRead'
import type { ReadTarget } from '@/lib/toRead'

interface ToReadProps {
  /** Distinct cards of the deck whose draft waits for a read (`summary.to_read`). */
  count: number
  /**
   * The deck whose cards are read; null links to every card (a deck that is not saved has no queue of its own), and
   * none draws no link, where a link cannot sit (a list option).
   */
  target?: ReadTarget | null
  className?: string
}

/**
 * "3 cards to read" with a book beside it, where a deck check says drafts wait for a player; nothing for none. With a
 * target it links to the review screen (`/read?deck=<id>`, `/read?precon=<code>` or `/read`, also in
 * `data-read-path`).
 */
export function ToRead({ count, target, className }: ToReadProps) {
  if (count <= 0) return null
  const look = cn('inline-flex items-center gap-1 font-medium text-foreground', className)
  const body = (
    <>
      <BookOpenIcon aria-hidden="true" className="size-4 shrink-0 text-support-read" />
      {cardsToRead(count)}
    </>
  )
  if (target === undefined) {
    return (
      <span data-slot="to-read" className={look}>
        {body}
      </span>
    )
  }
  return (
    <Link
      to="/read"
      search={target ?? {}}
      data-slot="to-read"
      data-read-path={target === null ? '/read' : readPath(target)}
      className={cn(look, 'underline-offset-4 hover:underline')}
      onClick={(event) => event.stopPropagation()}
    >
      {body}
    </Link>
  )
}
