import { cn } from '@/lib/utils'

/** The most dots drawn; a longer run says its place in words alone. */
const DOTS_MOST = 6

/**
 * The place of a question in its run of triggers, for the band: a dot per trigger (done, now, still to come) and "2 of
 * 3", so the next question in the same place is read as a new one.
 */
export function RunDots({ index, count }: { index: number; count: number }) {
  return (
    <span
      data-slot="run-dots"
      role="img"
      aria-label={`Trigger ${index} of ${count}`}
      className="inline-flex shrink-0 items-center gap-1 text-xs font-medium whitespace-nowrap text-muted-foreground tabular-nums"
    >
      {count > DOTS_MOST
        ? null
        : Array.from({ length: count }, (_, place) => (
            <i
              key={place}
              aria-hidden="true"
              data-state={place < index - 1 ? 'done' : place === index - 1 ? 'now' : 'next'}
              className={cn(
                'size-2 rounded-full border border-border-strong',
                place < index - 1 && 'border-transparent bg-success',
                place === index - 1 && 'border-transparent bg-turn-you ring-2 ring-turn-you/30',
              )}
            />
          ))}
      <span aria-hidden="true" className="ml-1">
        {index} of {count}
      </span>
    </span>
  )
}
