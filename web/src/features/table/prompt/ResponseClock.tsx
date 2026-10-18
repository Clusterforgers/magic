import { useDeferredValue, useState } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { useReducedMotion } from '@/hooks/useReducedMotion'
import { cn } from '@/lib/utils'
import type { DecisionId } from '@/protocol/generated'
import type { DecisionClock } from '@/play/store'
import type { DecisionName } from '../turnText'
import { clockAnnouncement, clockText, secondsLeft, URGENT_SECONDS } from './clockText'

interface ClockFaceProps {
  clock: DecisionClock
  kind: DecisionName
}

/** The face of a running clock; it is mounted once per decision, so what it starts from is what the prompt began with. */
function ClockFace({ clock, kind }: ClockFaceProps) {
  const reduced = useReducedMotion()
  // The bar runs on its own from where it starts; changing its duration every second would make it jump.
  const [start] = useState(() => ({
    ms: clock.leftMs,
    from: Math.min(1, clock.leftMs / clock.totalMs),
  }))
  // Said once, when the prompt appears: a countdown that announced itself every second would drown the game.
  const heard = useDeferredValue(clockAnnouncement(kind, secondsLeft(start.ms)), '')
  const seconds = secondsLeft(clock.leftMs)
  const urgent = seconds <= URGENT_SECONDS
  return (
    <div data-slot="response-clock" data-urgent={urgent || undefined} className="grid gap-1">
      <p
        role="timer"
        className={cn('text-xs font-medium', urgent ? 'text-destructive' : 'text-muted-foreground')}
      >
        {clockText(kind, seconds)}
      </p>
      {reduced ? null : (
        <div className="h-1 w-full overflow-hidden rounded-full bg-border" aria-hidden="true">
          <div
            data-slot="response-clock-bar"
            className="response-clock-bar h-full w-full rounded-full bg-primary"
            style={{ animationDuration: `${start.ms}ms`, ['--clock-from' as string]: start.from }}
          />
        </div>
      )}
      <p className="sr-only" role="status">
        {heard}
      </p>
    </div>
  )
}

interface ResponseClockProps {
  /** The decision the prompt answers; a clock that belongs to another is not shown. */
  id: DecisionId
  kind: DecisionName
}

/**
 * The countdown of a decision on a clock: the seconds left as text and, where motion is welcome, a bar that drains
 * over the time left. With a preference for less motion there is only the number, which still changes once a second.
 * It reads the session's clock, which counts from the moment the decision arrived and ends the prompt at zero, so
 * it draws nothing for a decision without one.
 */
export function ResponseClock({ id, kind }: ResponseClockProps) {
  const clock = usePlay((state) => state.clock)
  if (clock === null || clock.id !== id) return null
  return <ClockFace key={id} clock={clock} kind={kind} />
}
