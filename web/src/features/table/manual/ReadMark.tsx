/** How a card says its rules run on a draft: the mark on the face, "Read" once a player read it, else "Unverified". */
import { BookDashed, BookOpenCheck } from 'lucide-react'
import { ON_A_READ } from '@/lib/byHand'
import { UNVERIFIED, UNVERIFIED_MEANING } from '@/lib/toRead'
import { cn } from '@/lib/utils'
import type { ObjectView } from '@/protocol/generated'
import type { ByHandSpot } from './ByHand'

/**
 * The mark of a card whose rules run on a draft (`ObjectView.read`): an icon and "Read" in the `support-read` colour, or
 * "Unverified" with a dashed book in the `support-unverified` colour while no player has read it
 * (`ObjectView.unverified`); the icon alone on a small face. It only shows; the card's preview says more.
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
  const unverified = object.unverified === true
  const Icon = unverified ? BookDashed : BookOpenCheck
  return (
    <span
      data-slot="card-read"
      data-spot={spot}
      data-unverified={unverified || undefined}
      title={unverified ? UNVERIFIED_MEANING : ON_A_READ}
      aria-hidden="true"
      className={cn(
        'inline-flex items-center justify-center gap-1 rounded-full font-semibold whitespace-nowrap text-background shadow-sm',
        unverified ? 'bg-support-unverified' : 'bg-support-read',
        spot === 'chip' && compact ? 'h-5 min-w-5' : 'h-6 min-w-6',
        spot === 'corner' ? 'border border-background px-1' : 'px-2',
        text,
      )}
    >
      <Icon aria-hidden="true" className="size-3 shrink-0" />
      {compact ? null : <span>{unverified ? UNVERIFIED : 'Read'}</span>}
    </span>
  )
}
