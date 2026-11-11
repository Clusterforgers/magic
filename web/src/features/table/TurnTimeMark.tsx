import { Clock } from 'lucide-react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import { useSecondTick } from '@/hooks/useSecondTick'
import { formatTurnTime, turnTimeText } from '@/lib/turnTime'
import { cn } from '@/lib/utils'
import type { PlayerId } from '@/protocol/generated'
import { playerOf } from './selectors'

/**
 * The time a seat has used on its own turns, like a chess clock's tally: a clock icon and m:ss. Only the seat whose
 * turn runs ticks, on the shared one-second ticker; nothing shows before the seat's first turn.
 */
export function TurnTimeMark({ seat, className }: { seat: PlayerId; className?: string }) {
  const { ms, running, at } = usePlay(
    useShallow((state) => {
      const player = playerOf(state, seat)
      return {
        ms: player?.turn_time_ms ?? 0,
        running: player?.turn_time_running ?? false,
        at: state.viewAt,
      }
    }),
  )
  const now = useSecondTick(running)
  if (ms === 0 && !running) return null
  const used = running && at !== null ? ms + Math.max(0, now - at) : ms
  const text = turnTimeText(used)
  return (
    <span
      role="img"
      aria-label={text}
      title={text}
      data-slot="turn-time"
      data-running={running || undefined}
      className={cn(
        'inline-flex shrink-0 items-center gap-0.5 text-xs leading-4 tabular-nums',
        running ? 'text-foreground' : 'text-muted-foreground',
        className,
      )}
    >
      <Clock aria-hidden="true" className="size-3" />
      <span aria-hidden="true">{formatTurnTime(used)}</span>
    </span>
  )
}
