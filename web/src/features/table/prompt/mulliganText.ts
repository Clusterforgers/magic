/** The words of the Mulligan prompt: what the hand is, what a mulligan costs and who else is still deciding. */
import type { PlayState } from '@/play/store'
import type { MulliganOptions, PlayerId } from '@/protocol/generated'
import { cards } from './countedText'

/**
 * The question, the mulligans so far and the price of another. A mulligan draws a fresh hand and then puts one more
 * card than the mulligans taken so far on the bottom (the London mulligan; no format has a free mulligan yet).
 */
export function mulliganText(options: MulliganOptions): string {
  const { hand, mulligans_taken: taken, can_mulligan: can } = options
  const size = hand.length
  const thisHand = size === 0 ? 'this hand' : `this hand of ${cards(size)}`
  const soFar =
    taken === 0
      ? 'You have not taken a mulligan yet.'
      : `You have taken ${taken} ${taken === 1 ? 'mulligan' : 'mulligans'} so far.`
  if (!can) return `Keep ${thisHand}? ${soFar} You cannot take another.`
  const drawn = size === 0 ? 'a new hand' : `${size} new ${size === 1 ? 'card' : 'cards'}`
  const cost = `${taken === 0 ? 'A' : 'Another'} mulligan draws ${drawn}, then puts ${cards(taken + 1)} on the bottom of your library.`
  return `Keep ${thisHand}, or take a mulligan? ${soFar} ${cost}`
}

/** The other seats that have not finished the opening hand: still deciding to keep, or choosing cards to bottom. */
export function othersDeciding(state: PlayState): PlayerId[] {
  const { view, seat } = state
  if (view === null) return []
  return view.players
    .filter(
      (player) =>
        player.seat !== seat &&
        player.lost === null &&
        (player.mulligan === 'Deciding' || typeof player.mulligan !== 'string'),
    )
    .map((player) => player.seat)
}

/** "Still deciding: Bob and Cara." */
export function decidingText(names: readonly string[]): string {
  const list = [...names]
  const last = list.pop()
  return `Still deciding: ${list.length === 0 ? last : `${list.join(', ')} and ${last}`}.`
}
