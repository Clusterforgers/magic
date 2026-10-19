/** The marks of the step ladder: which steps are behind, which is now, which are ahead, and which just went unasked. */
import type { Step } from '@/protocol/generated'
import { STEP_LABEL } from '../turnText'

/** How a step of the ladder is drawn: still to come, behind, the current one, or just skipped (for a moment). */
export type PipState = 'upcoming' | 'passed' | 'current' | 'skipped'

// Every step in the order a turn runs them, from the keys of the step words so a new step cannot be left out.
const TURN_ORDER = Object.keys(STEP_LABEL) as Step[]

const place = (step: Step): number => TURN_ORDER.indexOf(step)

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
