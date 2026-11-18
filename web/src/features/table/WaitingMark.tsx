import { MessageCircleMore } from 'lucide-react'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { PlayerId } from '@/protocol/generated'
import { useSeatNamer } from './names'
import { waitingMarkText } from './waitingMarkText'

/**
 * The mark beside the name of a pod or tile whose player the game waits on, at a table of three or more: a speech
 * bubble with dots, named in words for a reader and in its tooltip, so it is never colour alone.
 */
export function WaitingMark({ seat, className }: { seat: PlayerId; className?: string }) {
  const name = useSeatNamer()
  const text = usePlay((state) => waitingMarkText(state, seat, name))
  if (text === null) return null
  return (
    <span
      role="img"
      aria-label={text}
      title={text}
      data-slot="waiting-mark"
      className={cn('inline-flex shrink-0 items-center text-foreground', className)}
    >
      <MessageCircleMore aria-hidden="true" className="size-3.5" />
    </span>
  )
}
