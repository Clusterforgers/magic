import { useEffect, useState } from 'react'
import { cn } from '@/lib/utils'
import type { TickerLine } from './feed'
import { TICKER_LINE_MS } from './timing'
import { useCueOptions, useTurnFeed } from './useTurnFeed'

interface Run {
  lines: readonly TickerLine[]
  index: number
  /** Tells one batch's run from the next, so a line that repeats still draws again. */
  id: number
}

/**
 * One line under the strip's content that says what the batch that just arrived did: the steps it skipped, the viewer's
 * own reason for a skip and the newest line worth reading, three lines at most, each for 3 s. A newer batch replaces
 * what is left of an older one at once, so the line is never behind the game; nothing in it can be pressed and nothing
 * waits for it. Its row keeps its height when empty, so the strip does not move as lines come and go.
 */
export function TurnTicker() {
  const { on, slow } = useCueOptions()
  const [run, setRun] = useState<Run | null>(null)
  useTurnFeed((feed) => {
    if (feed.lines.length > 0) setRun((was) => ({ lines: feed.lines, index: 0, id: (was?.id ?? 0) + 1 }))
  })
  useEffect(() => {
    if (run === null) return undefined
    const last = run.index + 1 >= run.lines.length
    const timer = setTimeout(
      () => setRun(last ? null : { ...run, index: run.index + 1 }),
      TICKER_LINE_MS * slow,
    )
    return () => clearTimeout(timer)
  }, [run, slow])
  if (!on) return null
  const line = run?.lines[run.index]
  return (
    <p
      data-slot="turn-ticker"
      aria-hidden="true"
      title={line?.text}
      className="h-4 min-w-0 truncate text-xs leading-4 text-muted-foreground"
    >
      {line === undefined ? null : (
        <span
          key={`${run?.id}:${run?.index}`}
          data-slot="turn-ticker-line"
          data-kind={line.kind}
          className={cn(line.kind !== 'event' && 'font-medium text-foreground')}
        >
          {line.text}
        </span>
      )}
    </p>
  )
}
