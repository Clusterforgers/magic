import { useMemo } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import type { AttackTarget, ObjectId, ObjectView } from '@/protocol/generated'
import { useSeatNamer } from '../names'
import type { TargetNames } from './combatText'

/** The objects behind `ids`, in order (undefined for one the view does not hold); the array changes only when one does. */
export function useObjects(ids: readonly ObjectId[]): readonly (ObjectView | undefined)[] {
  return usePlay(useShallow((state) => ids.map((id) => state.objectsById[id])))
}

/** How to call the defenders in `targets`: players by seat name, planeswalkers by card name. */
export function useTargetNames(targets: readonly AttackTarget[]): TargetNames {
  const seatName = useSeatNamer()
  const walkerIds = useMemo(
    () => targets.flatMap((target) => ('Planeswalker' in target ? [target.Planeswalker] : [])),
    [targets],
  )
  const walkers = useObjects(walkerIds)
  return useMemo(() => {
    const byId = new Map(walkers.flatMap((object) => (object === undefined ? [] : [[object.id, object]])))
    const short = (target: AttackTarget): string =>
      'Player' in target ? seatName(target.Player) : (byId.get(target.Planeswalker)?.name ?? 'a planeswalker')
    return {
      short,
      long(target) {
        if ('Player' in target) return short(target)
        const owner = byId.get(target.Planeswalker)?.controller
        return owner === undefined ? short(target) : `${short(target)}, ${seatName(owner)}'s planeswalker`
      },
    }
  }, [seatName, walkers])
}
