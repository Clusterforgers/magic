import { Gamepad2 } from 'lucide-react'
import { useShallow } from 'zustand/react/shallow'
import { Button } from '@/components/ui/button'
import { Popover, PopoverContent, PopoverTitle, PopoverTrigger } from '@/components/ui/popover'
import { usePlay } from '@/features/game/gameContext'
import { canSetStops } from '../selectors'
import { stopsButtonName } from './stopModel'
import { StopsBar } from './StopsBar'

/**
 * The stops in the controls dock: the ladder and Full Control in one line when the dock has the room (`row`), else one
 * "Stops" button that opens the same content as a popover above it. Never both. The popover stays in the page while
 * shut, so the Keep toggle and the picked side keep their state and the Control keys go on working.
 */
export function StopsDock({ row }: { row: boolean }) {
  const { shown, count, mode } = usePlay(
    useShallow((state) => ({
      shown: canSetStops(state),
      count: state.stops.length,
      mode: state.fullControl,
    })),
  )
  if (!shown) return null
  if (row) return <StopsBar layout="row" />
  return (
    <Popover>
      <PopoverTrigger
        render={
          <Button data-slot="stops-button" variant="outline" size="sm" aria-label={stopsButtonName(count, mode)} />
        }
      >
        <span aria-hidden="true">Stops</span>
        {count > 0 ? (
          <span aria-hidden="true" className="rounded bg-foreground/15 px-1 text-xs leading-4">
            {count}
          </span>
        ) : null}
        {mode === 'Off' ? null : <Gamepad2 aria-hidden="true" />}
      </PopoverTrigger>
      <PopoverContent keepMounted side="top" align="end" className="w-auto max-w-[calc(100vw-1rem)] gap-0 p-2">
        <PopoverTitle className="sr-only">Stops and Full Control</PopoverTitle>
        <StopsBar layout="panel" />
      </PopoverContent>
    </Popover>
  )
}
