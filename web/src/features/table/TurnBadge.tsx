import { useShallow } from 'zustand/react/shallow'
import { Pill } from '@/components/Pill'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import { useSeatNamer } from './names'
import { isYourTurn } from './selectors'
import { PhaseTag } from './turn/PhaseTag'
import { usePhase } from './turn/usePhase'
import { badgeText } from './turnText'

// Whole class names, so Tailwind can see them. The phase's colour fills the badge on your turn and tints it on another's.
// A block, not the pill's flex box, so that words which do not fit end in an ellipsis instead of being cut at both ends.
const ON_PHASE = {
  you: 'block w-auto rounded-l-none border-2 border-phase border-l-on-phase/30 bg-phase leading-6 text-on-phase',
  them: 'block w-auto rounded-l-none border-2 border-phase border-l-phase/40 bg-phase-soft leading-6 text-foreground',
}

/**
 * Whose turn it is and in which step, always in the top bar: "Your turn · 5 · Precombat main", "Bo's turn · 5 · Declare
 * blockers", and what the game is doing before and after its turns ("Opening hands"). During the turns the phase's
 * icon and name stand at its left in the phase's colour (`PhaseTag`): filled for your own turn, tinted for another's.
 * The heading of the table's moment, plain text and not a live region; `data-slot="banner"` is where scripts read it.
 */
export function TurnBadge() {
  const name = useSeatNamer()
  const input = usePlay(
    useShallow((state) => ({
      stage: state.view?.stage ?? 'PreGame',
      turn: state.view?.turn.number ?? 0,
      active: state.view?.turn.active ?? 0,
      step: state.view?.turn.step ?? 'Untap',
    })),
  )
  const seat = usePlay((state) => state.seat)
  const yours = usePlay(isYourTurn)
  const phase = usePhase()
  return (
    <div data-slot="turn-badge" className="flex h-7 min-w-0 items-center">
      {phase === null ? null : <PhaseTag phase={phase} yours={yours} />}
      <h2 data-slot="banner" data-your-turn={yours || undefined} className="min-w-0">
        <Pill
          data-themed={phase === null ? undefined : ''}
          className={cn(
            'h-7 max-w-full shrink truncate px-3 text-sm',
            phase !== null && ON_PHASE[yours ? 'you' : 'them'],
          )}
        >
          {badgeText(input, seat, name)}
        </Pill>
      </h2>
    </div>
  )
}
