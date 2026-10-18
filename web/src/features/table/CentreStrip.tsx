import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import { useSeatNamer } from './names'
import { bannerText, priorityText, waitingOn, waitingText } from './turnText'
import { useStable } from './useStable'

/**
 * The middle of the table: the turn banner, who has priority and, read-only, what the game is waiting for. The
 * decision prompts replace the last line with real controls; `data-slot="decision-line"` marks the place.
 */
export function CentreStrip() {
  const name = useSeatNamer()
  const banner = usePlay(
    useShallow((state) => ({
      stage: state.view?.stage ?? 'PreGame',
      turn: state.view?.turn.number ?? 0,
      active: state.view?.turn.active ?? 0,
      step: state.view?.turn.step ?? 'Untap',
    })),
  )
  const priority = usePlay((state) => state.view?.priority ?? null)
  const waiting = usePlay(useStable(waitingOn))
  const holder = priorityText(priority, banner.stage, name)
  return (
    <section
      aria-label="Turn and priority"
      className="flex flex-wrap items-center justify-between gap-x-6 gap-y-2 rounded-xl bg-card p-3 ring-1 ring-foreground/10"
    >
      <div className="grid gap-0.5">
        <h2 data-slot="banner" className="text-lg leading-tight font-semibold">
          {bannerText(banner, name)}
        </h2>
        {holder === '' ? null : (
          <p data-slot="priority" className="text-sm">
            {holder}
          </p>
        )}
      </div>
      <p
        data-slot="decision-line"
        aria-live="polite"
        aria-atomic="true"
        className="w-fit rounded-md bg-muted px-2 py-1 text-sm font-medium"
      >
        {waitingText(waiting, name)}
      </p>
    </section>
  )
}
