/** Whether a seat has nothing to play from its hand dock: no card in hand, no commander waiting and no top card to cast. */
import type { PlayState } from '@/play/store'
import type { PlayerId } from '@/protocol/generated'
import { commandersInHand } from './HandDock'
import { seenTop } from './libraryTop'
import { playerOf } from './selectors'

/** True when the hand dock of `seat` holds only the words "Your hand is empty": a Spirit's, or a hand played out. */
export function handIdle(state: PlayState, seat: PlayerId): boolean {
  const player = playerOf(state, seat)
  if (player === undefined) return false
  if (player.hand.count > 0 || seenTop(player) !== undefined) return false
  return commandersInHand(player, state.objectsById).size === 0
}
