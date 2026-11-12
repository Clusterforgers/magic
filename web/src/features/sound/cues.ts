/** Which sound cue, if any, a change of the play store calls for: pure, so it is tested without audio. */
import type { PlayState } from '@/play/store'

/** A short sound: the viewer's turn began, a decision waits for the viewer, or the viewer won. */
export type SoundCue = 'turn' | 'decision' | 'won'

/**
 * The cue for the change from `prev` to `next`, or null. At most one plays per change, the strongest first: a win, then
 * the start of the viewer's own turn, then a decision that arrives while none was pending. A first view (a join or a
 * reconnect) and a change of another seat play nothing, and each cue reads only what the viewer's own store holds.
 */
export function soundCueBetween(prev: PlayState, next: PlayState): SoundCue | null {
  const seat = next.seat
  if (seat === null || next.view?.spectator === true) return null
  if (prev.result === null && next.result !== null && next.result !== 'Draw' && next.result.Winner === seat)
    return 'won'
  const before = prev.view?.turn
  const after = next.view?.turn
  if (before !== undefined && after !== undefined && after.number !== before.number && after.active === seat)
    return 'turn'
  if (
    prev.view !== null &&
    prev.pending === null &&
    next.pending !== null &&
    next.pending.decision.player === seat
  )
    return 'decision'
  return null
}
