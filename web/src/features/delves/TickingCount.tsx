import { cn } from '@/lib/utils'
import { useTick } from './beat'

/** A number that pops into its new value each time it changes (votes, gold), never on its first render. */
export function TickingCount({ value, className }: { value: number; className?: string }) {
  const tick = useTick(value)
  return (
    <strong
      // A new key mounts the figure afresh, so its entrance plays again.
      key={tick}
      data-slot="ticking-count"
      data-tick={tick > 0 || undefined}
      className={cn(
        'inline-block tabular-nums',
        tick > 0 && 'motion-safe:animate-in motion-safe:zoom-in-150 motion-safe:duration-base',
        className,
      )}
    >
      {value}
    </strong>
  )
}
