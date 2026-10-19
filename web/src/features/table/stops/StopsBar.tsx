import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import { canSetStops } from '../selectors'
import { FullControlToggle } from './FullControlToggle'
import { StopLadder } from './StopLadder'
import type { StopsLayout } from './stopModel'

/**
 * The stop ladder with the Full Control toggle beside it, for a seat that is still in a running game: one line in the
 * controls dock (`row`), or wrapped for the popover that opens from the dock's "Stops" button (`panel`).
 */
export function StopsBar({ layout }: { layout: StopsLayout }) {
  const shown = usePlay(canSetStops)
  if (!shown) return null
  return (
    <section
      aria-label="Stops and Full Control"
      data-slot="stops-bar"
      data-layout={layout}
      className={cn('flex items-center gap-2', layout === 'row' ? 'flex-nowrap' : 'flex-wrap justify-end')}
    >
      <StopLadder layout={layout} />
      <FullControlToggle />
    </section>
  )
}
