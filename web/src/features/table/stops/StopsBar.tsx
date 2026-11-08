import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import { canSetStops } from '../selectors'
import { StepLadder } from '../turn/StepLadder'
import { FullControlToggle } from './FullControlToggle'
import { StopLadder } from './StopLadder'
import type { StopsLayout } from './stopModel'

/**
 * The stops' controls (side switch and Keep) with the Full Control toggle beside them, for a seat that is still in a
 * running game: one line in the controls dock (`row`), or wrapped for the popover that opens from the dock's "Stops"
 * button (`panel`). The popover also holds the step ladder when the top bar has no room for it.
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
      {layout === 'panel' ? (
        <div className="basis-full">
          <StepLadder place="panel" />
        </div>
      ) : null}
    </section>
  )
}
