import type { PendingDecision, PlayState } from '@/play/store'

/** The pending decision when it is this seat's to answer, else null. */
export function ownDecision(state: PlayState): PendingDecision | null {
  const { pending, seat } = state
  return pending !== null && seat !== null && pending.decision.player === seat ? pending : null
}

/**
 * True while this seat must answer a decision whose prompt and buttons are in the shared strip: every kind but
 * Priority, whose passing buttons are in the controls dock and whose line only says what can be done.
 */
export function promptInStrip(state: PlayState): boolean {
  const own = ownDecision(state)
  return own !== null && !('Priority' in own.decision.kind)
}
