/** The words of the Mulligan prompt: what the hand is, what keeping and a mulligan cost and who else is still deciding. */
import type { PlayState } from '@/play/store'
import type { MulliganOptions, PlayerId } from '@/protocol/generated'
import { cards } from './countedText'

/**
 * The question, the mulligans so far and how many cards keeping or another mulligan puts on the bottom of the
 * library. The counts come from the engine's options (the format decides which mulligans are free); nothing is
 * worked out here.
 */
export function mulliganText(options: MulliganOptions): string {
  const { hand, mulligans_taken: taken, can_mulligan: can } = options
  const { bottom_if_keep: keep, bottom_if_mulligan: next, free_mulligans: freeAllowed } = options
  const size = hand.length
  const newSize = options.hand_if_mulligan ?? size
  const scryKeep = options.scry_if_keep ?? 0
  const scryNext = options.scry_if_mulligan ?? 0
  const scried = (count: number): string => (count === 0 ? '' : ` then scries ${count}`)
  const thisHand = size === 0 ? 'this hand' : `this hand of ${cards(size)}`
  const free = Math.min(taken, freeAllowed)
  const soFar =
    taken === 0
      ? 'You have not taken a mulligan yet.'
      : `You have taken ${taken} ${taken === 1 ? 'mulligan' : 'mulligans'} so far${free === 0 ? '' : ` (${free} free)`}.`
  const keeping =
    (keep === 0 ? '' : ` Keeping it puts ${cards(keep)} on the bottom of your library.`) +
    (scryKeep === 0 ? '' : ` Keeping it also scries ${scryKeep}.`)
  if (!can) return `Keep ${thisHand}? ${soFar}${keeping} You cannot take another.`
  const drawn = newSize === 0 ? 'a new hand' : `${newSize} new ${newSize === 1 ? 'card' : 'cards'}`
  const another = taken === 0 ? 'A' : 'Another'
  const cost =
    newSize < size
      ? `${another} mulligan deals a smaller hand of ${cards(newSize)} and puts nothing on the bottom of your library${scried(scryNext)}.`
      : next > keep
        ? `${another} mulligan draws ${drawn}, then puts ${cards(next)} on the bottom of your library${scried(scryNext)}.`
        : `${another} mulligan is free: it draws ${drawn} and adds no card to the bottom of your library${scried(scryNext)}.`
  return `Keep ${thisHand}, or take a mulligan? ${soFar}${keeping} ${cost}`
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
