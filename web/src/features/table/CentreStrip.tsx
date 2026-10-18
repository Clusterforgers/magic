import { useShallow } from 'zustand/react/shallow'
import { Badge } from '@/components/ui/badge'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import { CombatLegend } from './combat/CombatLegend'
import { useSeatNamer } from './names'
import { OpeningStatus } from './OpeningStatus'
import { NothingToDoBanner } from './prompt/NothingToDoBanner'
import { ownDecision } from './prompt/ownDecision'
import { PromptHost } from './prompt/PromptHost'
import { isYourTurn } from './selectors'
import { bannerText, priorityText, waitingOn, waitingText } from './turnText'
import { useStable } from './useStable'
import { YieldIndicator } from './YieldIndicator'

/**
 * The middle of the table: the turn banner, who has priority and what the game is waiting for. When this seat must
 * decide, the prompt (`prompt/PromptHost`) takes the place of the read-only line; `data-slot="decision-line"` marks
 * both. A "nothing to do" note sits beneath.
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
  const mine = usePlay((state) => ownDecision(state) !== null)
  const holder = priorityText(priority, banner.stage, name)
  const yourTurn = usePlay(isYourTurn)
  return (
    <section
      aria-label="Turn and priority"
      data-your-turn={yourTurn || undefined}
      className={cn(
        'flex flex-wrap items-center justify-between gap-x-6 gap-y-2 rounded-xl bg-card p-3',
        yourTurn ? 'your-turn-cue' : 'ring-1 ring-foreground/10',
      )}
    >
      <div className="grid gap-0.5">
        <div className="flex flex-wrap items-center gap-2">
          <h2 data-slot="banner" className="text-lg leading-tight font-semibold">
            {bannerText(banner, name)}
          </h2>
          {yourTurn ? <Badge data-slot="your-turn">Your turn</Badge> : null}
        </div>
        {holder === '' ? null : (
          <p data-slot="priority" className="text-sm">
            {holder}
          </p>
        )}
      </div>
      {mine ? (
        <PromptHost />
      ) : (
        <p
          data-slot="decision-line"
          aria-live="polite"
          aria-atomic="true"
          className="w-fit rounded-md bg-muted px-2 py-1 text-sm font-medium"
        >
          {waitingText(waiting, name)}
        </p>
      )}
      <OpeningStatus />
      <YieldIndicator />
      <CombatLegend />
      <NothingToDoBanner />
    </section>
  )
}
