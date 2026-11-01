/**
 * Quick cast: while the server passes for this seat (no decision of its own on screen), an instant or a flash card in
 * hand stays clickable; a click asks for the seat's next priority window and casts the card there.
 */
import type { PlayState } from '@/play/store'
import type { ObjectId, ObjectView } from '@/protocol/generated'
import { canSetStops, playerOf } from '../selectors'
import { ownDecision } from './ownDecision'

/** Whether `object` may be cast at any time a player gets priority: an instant or a card with flash. */
export function castsAnyTime(object: ObjectView): boolean {
  return object.types.includes('Instant') || (object.keywords ?? []).includes('Flash')
}

/**
 * The cards of this seat's hand a click may queue for its next window, in hand order: none while a decision of its
 * own is on screen (that prompt owns the clicks) or once the seat is out of a running game.
 */
export function quickCastIds(state: PlayState): ObjectId[] {
  const { seat } = state
  if (seat === null || ownDecision(state) !== null || !canSetStops(state)) return []
  const hand = playerOf(state, seat)?.hand.cards ?? []
  return hand.filter((id) => {
    const object = state.objectsById[id]
    return object !== undefined && castsAnyTime(object)
  })
}

/** What the card says to a screen reader and on hover: whether it waits for the next window or a click queues it. */
export function quickCastNote(queued: boolean): { words: string; hint: string; line: string } {
  return queued
    ? {
        words: 'cast at your next priority, press again to cancel',
        hint: 'Casting next',
        line: 'Cast when you next get priority. Click again to cancel.',
      }
    : {
        words: 'click to cast at your next priority',
        hint: 'Cast next',
        line: 'Click to get priority at the next chance and cast it then.',
      }
}
