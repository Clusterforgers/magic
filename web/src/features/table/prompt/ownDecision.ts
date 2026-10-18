import type { PendingDecision, PlayState } from '@/play/store'

/** The pending decision when it is this seat's to answer, else null. */
export function ownDecision(state: PlayState): PendingDecision | null {
  const { pending, seat } = state
  return pending !== null && seat !== null && pending.decision.player === seat ? pending : null
}
