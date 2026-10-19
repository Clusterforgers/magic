/** The marks of the step ladder: which steps are behind, which is now, which are ahead, and which just went unasked. */
import type { Step } from '@/protocol/generated'
import { STEP_LABEL } from '../turnText'
import { phaseOf } from './phase'
import type { Phase } from './phase'

/** How a step of the ladder is drawn: still to come, behind, the current one, or just skipped (for a moment). */
export type PipState = 'upcoming' | 'passed' | 'current' | 'skipped'

// Every step in the order a turn runs them, from the keys of the step words so a new step cannot be left out.
const TURN_ORDER = Object.keys(STEP_LABEL) as Step[]

const place = (step: Step): number => TURN_ORDER.indexOf(step)

/** Steps of the ladder that follow one another in one phase. */
export interface PhaseRun<T extends Step> {
  phase: Phase
  steps: T[]
}

/** `steps` cut into runs of one phase, in the order given; the main phase comes twice, around combat. */
export function phaseRuns<T extends Step>(steps: readonly T[]): Array<PhaseRun<T>> {
  const runs: Array<PhaseRun<T>> = []
  for (const step of steps) {
    const phase = phaseOf(step)
    const last = runs.at(-1)
    if (last?.phase.id === phase.id) last.steps.push(step)
    else runs.push({ phase, steps: [step] })
  }
  return runs
}

/**
 * The state of each of `steps` while the turn is in `current`: steps before it are passed, the step itself is
 * current, the rest are to come. A passed step that is in `dashed` shows as skipped until the caller lets it settle.
 */
export function pipStates(steps: readonly Step[], current: Step, dashed: readonly Step[]): PipState[] {
  const now = place(current)
  return steps.map((step) => {
    const at = place(step)
    if (at > now) return 'upcoming'
    if (at === now) return 'current'
    return dashed.includes(step) ? 'skipped' : 'passed'
  })
}
