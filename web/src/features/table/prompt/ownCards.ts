import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import type { PlayState } from '@/play/store'
import type { ObjectId } from '@/protocol/generated'
import { playerOf, splitBattlefield } from '../selectors'

/** The cards this seat holds in hand, keeps in its command zone (its commanders, cast from the hand dock) and controls on the battlefield; wrap in `useShallow`. */
export function ownCardIds(state: PlayState): ObjectId[] {
  const { seat, view } = state
  if (seat === null || view === null) return []
  const player = playerOf(state, seat)
  const hand = [...(player?.command.cards ?? []), ...(player?.hand.cards ?? [])]
  const rows = splitBattlefield(view, state.objectsById).get(seat)
  return rows === undefined ? hand : [...hand, ...rows.creatures, ...rows.other, ...rows.lands]
}

/** The ids of this seat's hand and battlefield cards; the array changes only when the ids do. */
export function useOwnCardIds(): readonly ObjectId[] {
  return usePlay(useShallow(ownCardIds))
}
