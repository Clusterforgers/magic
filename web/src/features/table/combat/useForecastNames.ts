import { useMemo } from 'react'
import { usePlay } from '@/features/game/gameContext'
import type { CombatForecast, ObjectId } from '@/protocol/generated'
import { useSeatNamer } from '../names'
import { uniqueNames } from '../prompt/combatText'
import { useObjects } from '../prompt/useCombatNames'
import type { ForecastNames } from './forecastText'

/** Every card a forecast names: attackers, their blockers, the creatures it lists and attacked planeswalkers. */
export function cardsOf(forecast: CombatForecast | null): ObjectId[] {
  const ids = new Set<ObjectId>()
  for (const attack of forecast?.attackers ?? []) {
    ids.add(attack.attacker)
    for (const blocker of attack.blockers) ids.add(blocker)
    if ('Planeswalker' in attack.target) ids.add(attack.target.Planeswalker)
  }
  for (const creature of forecast?.creatures ?? []) ids.add(creature.object)
  return [...ids]
}

/**
 * How to call what a forecast names, for sentences: "Bo's Hill Giant", "your Grizzly Bears" (two of one name get a
 * number), "you" for the viewer's seat and the seat's name for the rest.
 */
export function useForecastNames(forecast: CombatForecast | null): ForecastNames {
  const seatName = useSeatNamer()
  const viewer = usePlay((state) => state.seat)
  const ids = useMemo(() => cardsOf(forecast), [forecast])
  const objects = useObjects(ids)
  return useMemo(() => {
    const player = (seat: number): string => (seat === viewer ? 'you' : seatName(seat))
    const labels = uniqueNames(
      ids.map((id, index) => {
        const object = objects[index]
        if (object === undefined) return { id, name: 'a creature' }
        const owner = object.controller === viewer ? 'your' : `${seatName(object.controller)}'s`
        return { id, name: `${owner} ${object.name}` }
      }),
    )
    const creature = (id: ObjectId): string => labels.get(id) ?? 'a creature'
    return {
      creature,
      player,
      target: (target) => ('Player' in target ? player(target.Player) : creature(target.Planeswalker)),
    }
  }, [ids, objects, seatName, viewer])
}
