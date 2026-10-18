import { usePlay } from '@/features/game/gameContext'
import { canSetStops } from '../selectors'
import { FullControlToggle } from './FullControlToggle'
import { StopLadder } from './StopLadder'

/** The stop ladder with the Full Control toggle beside it, for a seat that is still in a running game. */
export function StopsBar() {
  const shown = usePlay(canSetStops)
  if (!shown) return null
  return (
    <section
      aria-label="Stops and Full Control"
      data-slot="stops-bar"
      className="flex flex-wrap items-center gap-x-3 gap-y-2 rounded-lg bg-card px-2 py-1.5 ring-1 ring-foreground/10"
    >
      <StopLadder />
      <FullControlToggle />
    </section>
  )
}
