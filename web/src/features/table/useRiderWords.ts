/** The words of the riders that mana in a pool carries, read from the permanents that made the mana. */
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import type { ManaPool as ManaPoolValue } from '@/protocol/generated'
import { manaRiders, riderKey, riderWords } from './manaRider'

/** The words of each rider in the pool by `riderKey`; a new map only when a sentence changes. */
export function useRiderWords(pool: ManaPoolValue): ReadonlyMap<string, string> {
  const riders = manaRiders(pool).flatMap((unit) => unit.riders)
  const found = usePlay(
    useShallow((state) =>
      riders.flatMap((rider) => [
        riderKey(rider),
        riderWords(rider, rider.source === undefined ? undefined : state.objectsById[rider.source]),
      ]),
    ),
  )
  return new Map(
    Array.from({ length: found.length / 2 }, (_, at): [string, string] => [
      found[2 * at] ?? '',
      found[2 * at + 1] ?? '',
    ]),
  )
}
