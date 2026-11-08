import { Pin } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { StopSideSwitch } from './StopSideSwitch'
import { stopsOn } from './stopModel'
import type { StopsLayout } from './stopModel'
import { useStopControls } from './stopControls'

/**
 * The stops' own controls beside the step ladder: "Mine" or "Theirs", which side's stops the ladder shows and sets
 * (it follows the turn, and a side picked by hand stays until the turn changes hands), and the Keep toggle, which makes
 * a plain press on a step set a lasting stop (the way to do it on touch, where there is no shift). The steps
 * themselves are the marks of the step ladder (`turn/StepLadder`).
 */
export function StopLadder({ layout }: { layout: StopsLayout }) {
  const controls = useStopControls()
  if (controls === null || !controls.shown) return null
  const { stops, turn, showing, keep } = controls
  return (
    <div
      role="group"
      aria-label="Stops"
      data-slot="stop-ladder"
      data-layout={layout}
      className={cn('flex items-center gap-2', layout === 'row' ? 'flex-nowrap' : 'flex-wrap')}
    >
      <StopSideSwitch
        shown={showing}
        turn={turn}
        counts={{ Own: stopsOn(stops, 'Own'), Opponent: stopsOn(stops, 'Opponent') }}
        onPick={controls.pick}
      />
      <Button
        data-slot="keep-stops"
        size="sm"
        variant="outline"
        className={cn(
          'px-2 pointer-coarse:px-3 aria-pressed:bg-primary aria-pressed:text-primary-foreground',
          layout === 'panel' && 'px-3',
        )}
        aria-pressed={keep}
        aria-label="Keep stops"
        aria-description="While on, pressing a step sets a stop that stays every turn."
        title="Keep stops: while on, pressing a step sets a stop that stays every turn"
        onClick={() => controls.setKeep(!keep)}
      >
        <Pin aria-hidden="true" />
        <span className={layout === 'panel' ? undefined : 'hidden pointer-coarse:inline'}>Keep</span>
      </Button>
    </div>
  )
}
