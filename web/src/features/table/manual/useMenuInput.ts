import { useMemo } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import type { CounterKind, ObjectId } from '@/protocol/generated'
import { useSeatNamer } from '../names'
import { playerOf } from '../selectors'
import { GAME_OVER, OUT_OF_GAME } from './offers'
import type { MenuInput } from './offers'

const NO_COUNTERS: readonly (readonly [CounterKind, number])[] = []

/** What the commands of card `id` are made from, as of now; null while the card or the seat is not on the table. */
export function useMenuInput(id: ObjectId): MenuInput | null {
  const nameOf = useSeatNamer()
  const seat = usePlay((state) => state.seat)
  const object = usePlay((state) => state.objectsById[id])
  const hasCommandZone = usePlay((state) => state.view?.format.toLowerCase() === 'commander')
  const mine = usePlay(
    useShallow((state) => {
      const player = seat === null ? undefined : playerOf(state, seat)
      return {
        library: player?.library.count ?? 0,
        counters: player?.counters ?? NO_COUNTERS,
        off: state.result !== null ? GAME_OVER : (player?.lost ?? null) !== null ? OUT_OF_GAME : undefined,
      }
    }),
  )
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
            off: mine.off,
            players,
            hasCommandZone,
            library: mine.library,
            counters: mine.counters,
            ownerName: nameOf(object.owner),
          },
    [seat, object, players, hasCommandZone, mine, nameOf],
  )
}
