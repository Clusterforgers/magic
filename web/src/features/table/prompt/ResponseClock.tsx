import { useDeferredValue, useState } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { useReducedMotion } from '@/hooks/useReducedMotion'
import { cn } from '@/lib/utils'
import type { DecisionId } from '@/protocol/generated'
import type { DecisionClock } from '@/play/store'
import { clockAnnouncement, clockText, secondsLeft, URGENT_SECONDS } from './clockText'
import type { ClockKind } from './clockText'

interface ClockFaceProps {
  clock: DecisionClock
  kind: ClockKind
  /** The words sit in the prompt's row and the bar is a 2 px line on the bottom edge of the decision line, which is positioned. */
  compact: boolean
}

/** The face of a running clock; it is mounted once per decision, so what it starts from is what the prompt began with. */
function ClockFace({ clock, kind, compact }: ClockFaceProps) {
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
    <div
      data-slot="response-clock"
      data-urgent={urgent || undefined}
      className={compact ? 'shrink-0' : 'grid gap-1'}
    >
      <p
        role="timer"
        className={cn('text-xs font-medium', urgent ? 'text-destructive' : 'text-muted-foreground')}
      >
        {clockText(kind, seconds)}
      </p>
      {reduced ? null : (
        <div
          className={cn(
            'overflow-hidden bg-border',
            compact ? 'absolute inset-x-0 bottom-0 h-0.5' : 'h-1 w-full rounded-full',
          )}
          aria-hidden="true"
        >
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
  kind: ClockKind
  /** One thin line: the seconds beside the prompt's controls and the bar on the edge (the combat prompts). */
  compact?: boolean
}

/**
 * The countdown of a decision on a clock: the seconds left as text and, where motion is welcome, a bar that drains
 * over the time left. With a preference for less motion there is only the number, which still changes once a second.
 * It reads the session's clock, which counts from the moment the decision arrived and ends the prompt at zero, so
 * it draws nothing for a decision without one.
 */
export function ResponseClock({ id, kind, compact = false }: ResponseClockProps) {
  const clock = usePlay((state) => state.clock)
  if (clock === null || clock.id !== id) return null
  return <ClockFace key={id} clock={clock} kind={kind} compact={compact} />
}
