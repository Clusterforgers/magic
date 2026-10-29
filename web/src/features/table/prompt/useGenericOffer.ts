import { useMemo } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { useSeatNamer } from '../names'
import { askerOf, genericChoices, namedObjects } from './genericChoices'
import type { GenericOffer } from './genericChoices'
import { usePrompt } from './PromptContext'
import { useObjects } from './useCombatNames'

/**
 * The answers on offer for the decision being faced (`genericChoices`), with the names of the cards they mention taken
 * from the view. Meant for a decision whose kind has no prompt of its own.
 */
export function useGenericOffer(): GenericOffer {
  const { decision } = usePrompt()
  const seatName = useSeatNamer()
  const viewer = usePlay((state) => state.seat)
  const { kind } = decision
  const asker = askerOf(kind)
  const ids = useMemo(() => [...(asker === undefined ? [] : [asker]), ...namedObjects(kind)], [asker, kind])
  const objects = useObjects(ids)
  return useMemo(() => {
    const names = new Map(
      objects.flatMap((object) => (object === undefined ? [] : [[object.id, object.name]])),
    )
    const source = asker === undefined ? undefined : objects[0]
    return genericChoices(kind, {
      object: (id) => names.get(id) ?? 'a card',
      seat: seatName,
      viewer,
      cancel: source?.zone === 'Battlefield' ? 'Cancel the ability' : 'Cancel the spell',
    })
  }, [kind, objects, asker, seatName, viewer])
}
