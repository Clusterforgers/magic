import { useMemo } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import type { ObjectId } from '@/protocol/generated'
import { useSeatNamer } from '../names'
import { playerOf } from '../selectors'
import { GAME_OVER, OUT_OF_GAME } from './offers'
import type { MenuInput } from './offers'

/** What the commands of card `id` are made from, as of now; null while the card or the seat is not on the table. */
export function useMenuInput(id: ObjectId): MenuInput | null {
  const nameOf = useSeatNamer()
  const seat = usePlay((state) => state.seat)
  const object = usePlay((state) => state.objectsById[id])
  const hasCommandZone = usePlay((state) => state.view?.format.toLowerCase() === 'commander')
  // A string or nothing: the menu follows the seat's standing in the game, not its numbers.
  const off = usePlay((state) => {
    const player = seat === null ? undefined : playerOf(state, seat)
    return state.result !== null ? GAME_OVER : (player?.lost ?? null) !== null ? OUT_OF_GAME : undefined
  })
  const players = usePlay(
    useShallow((state) => (state.view?.players ?? []).filter((p) => p.lost === null).map((p) => p.seat)),
  )
  return useMemo(
    () =>
      object === undefined || seat === null
        ? null
        : {
            seat,
            object,
            off,
            players,
            hasCommandZone,
            ownerName: nameOf(object.owner),
          },
    [seat, object, players, hasCommandZone, off, nameOf],
  )
}
