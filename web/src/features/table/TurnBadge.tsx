import { useShallow } from 'zustand/react/shallow'
import { Pill } from '@/components/Pill'
import type { PillTone } from '@/components/Pill'
import { usePlay } from '@/features/game/gameContext'
import { useSeatNamer } from './names'
import { isYourTurn } from './selectors'
import { badgeText } from './turnText'

/**
 * Whose turn it is and in which step, always in the top bar: "Your turn · 5 · Precombat main" on a gold pill, "Bo's turn ·
 * 5 · Declare blockers" on an outlined one, and what the game is doing before and after its turns. Plain text, not a live
 * region; `data-slot="banner"` is where scripts read the moment of the game.
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
  const tone: PillTone = yours ? 'turn-you' : input.stage === 'Turns' ? 'turn-them' : 'neutral'
  return (
    <p data-slot="banner" data-your-turn={yours || undefined} className="min-w-0">
      <Pill tone={tone} className="h-7 max-w-full shrink truncate px-3 text-sm">
        {badgeText(input, seat, name)}
      </Pill>
    </p>
  )
}
