import { useMediaQuery } from '@/hooks/useMediaQuery'
import { anchor } from '@/features/learn/anchors'
import { Gamepad2 } from 'lucide-react'
import { useShallow } from 'zustand/react/shallow'
import { Button } from '@/components/ui/button'
import { Popover, PopoverContent, PopoverTitle, PopoverTrigger } from '@/components/ui/popover'
import { usePlay } from '@/features/game/gameContext'
import { canSetStops } from '../selectors'
import { stopsButtonName } from './stopModel'
import { AutoSkipToggle } from './AutoSkipToggle'
import { PassTurnToggle } from './PassTurnToggle'
import { StopsBar } from './StopsBar'

/**
 * The stops in the controls dock: the ladder and Full Control in one line when the dock has the room (`row`), else one
 * "Stops" button that opens the same content as a popover above it. Never both. The popover stays in the page while
 * shut, so the Keep toggle and the picked side keep their state and the Control keys go on working. The "Auto-skip
 * triggers" switch stays in sight beside Full Control in the row and beside the Stops button on a wider dock; a phone's dock
 * has no room for it beside the Stops button and the primary button, so there it is in the popover with Full Control.
 * A short dock tucks what one line beside the button cannot hold (`tuck`): under a question both switches (`all`), and
 * where its controls fold the Auto-skip group alone (`skip`), "Pass until my turn" staying beside the button.
 */
export function StopsDock({ row, tuck }: { row: boolean; tuck?: 'all' | 'skip' }) {
  const { shown, count, mode } = usePlay(
    useShallow((state) => ({
      shown: canSetStops(state),
      count: state.stops.length,
      mode: state.fullControl,
    })),
  )
  const phone = useMediaQuery('(max-width: 639px)', false)
  const tucked = phone ? 'all' : tuck
  if (!shown) return null
  if (row) return <StopsBar layout="row" />
  return (
    <>
      <Popover>
        <PopoverTrigger
          render={
            <Button
              data-slot="stops-button"
              {...anchor('stops.open')}
              variant="outline"
              size="sm"
              aria-label={stopsButtonName(count, mode)}
            />
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
        <PopoverContent
          keepMounted
          side="top"
          align="end"
          className="w-auto max-w-[calc(100vw-1rem)] gap-0 p-2"
        >
          <PopoverTitle className="sr-only">Stops and Full Control</PopoverTitle>
          <StopsBar layout="panel" />
          {tucked === undefined ? null : (
            <div className="mt-2 flex justify-end">
              <AutoSkipToggle passTurn={tucked === 'all'} />
            </div>
          )}
        </PopoverContent>
      </Popover>
      {tucked === undefined ? <AutoSkipToggle /> : null}
      {tucked === 'skip' ? <PassTurnToggle /> : null}
    </>
  )
}
