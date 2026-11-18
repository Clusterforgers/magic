import type { PlayState } from '@/play/store'
import type { PlayerId } from '@/protocol/generated'
import type { SeatNamer } from './names'
import { waitingOn, waitingText } from './turnText'

/** The fewest seats at which pods carry the mark: in a duel the strip's line names the one other player already. */
export const WAITING_MARK_SEATS = 3

/**
 * What the mark on `seat`'s pod says, "Waiting for Bo to choose targets", or null when the game does not wait on that
 * seat (the strip's `waitingOn`: never the viewer's own decision, which is the prompt) or the table has under three seats.
 */
export function waitingMarkText(state: PlayState, seat: PlayerId, name: SeatNamer): string | null {
  if ((state.view?.players.length ?? 0) < WAITING_MARK_SEATS) return null
  const waiting = waitingOn(state)
  if (waiting.kind !== 'others' || !waiting.seats.includes(seat)) return null
  return waitingText({ ...waiting, seats: [seat] }, name)
}
