import { usePlay } from '@/features/game/gameContext'
import { PHASES, phaseOf } from './phase'
import type { Phase } from './phase'

/**
 * The phase the turn is in, or `null` before the turns begin and after they end. It reads the stage and the step and
 * nothing else, which every seat sees, so every seat gets the same phase; a change of step is the change of phase,
 * with no timer between.
 */
export function usePhase(): Phase | null {
  const id = usePlay((state) => (state.view?.stage === 'Turns' ? phaseOf(state.view.turn.step).id : null))
  return id === null ? null : PHASES[id]
}
