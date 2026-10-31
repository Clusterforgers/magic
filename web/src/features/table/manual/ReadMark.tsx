/** How a card says its rules run on a player's read: the mark on the face. */
import { BookOpenCheck } from 'lucide-react'
import { ON_A_READ } from '@/lib/byHand'
import { cn } from '@/lib/utils'
import type { ObjectView } from '@/protocol/generated'
import type { ByHandSpot } from './ByHand'

/**
 * The mark of a card whose rules a player read and found correct (`ObjectView.read`): an icon and "Read" in the
 * `support-read` colour, the icon alone on a small face. It only shows; the card's preview says more.
 */
export function ReadMark({
  object,
  spot,
  compact,
  text,
}: {
  object: ObjectView
  spot: ByHandSpot
  compact: boolean
  text: string
}) {
  if (object.read !== true) return null
  return (
    <span
      data-slot="card-read"
      data-spot={spot}
      title={ON_A_READ}
      aria-hidden="true"
      className={cn(
        'inline-flex items-center justify-center gap-1 rounded-full bg-support-read font-semibold whitespace-nowrap text-background shadow-sm',
        spot === 'chip' && compact ? 'h-5 min-w-5' : 'h-6 min-w-6',
        spot === 'corner' ? 'border border-background px-1' : 'px-2',
        text,
      )}
    >
      <BookOpenCheck aria-hidden="true" className="size-3 shrink-0" />
      {compact ? null : <span>Read</span>}
    </span>
  )
}
