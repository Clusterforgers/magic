/**
 * The card a cast or a land play this seat just sent takes out of its hand (docs/ui-design.md, Feel, Doherty): it leaves
 * on the press, before the server's batch moves it, and comes back when the server refuses the answer.
 */
import type { PlayState } from '@/play/store'
import type { ObjectId } from '@/protocol/generated'

/** The card the answer in flight casts or plays as a land, or null while none is. */
export function leavingCard(state: Pick<PlayState, 'answered'>): ObjectId | null {
  const answer = state.answered
  if (answer === null || typeof answer !== 'object' || !('Priority' in answer)) return null
  const action = answer.Priority
  if (typeof action !== 'object') return null
  if ('CastSpell' in action) return action.CastSpell
  if ('PlayLand' in action) return action.PlayLand
  if ('PlayLandFace' in action) return action.PlayLandFace[0]
  return null
}
