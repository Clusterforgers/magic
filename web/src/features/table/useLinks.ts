/** What a card names besides itself, read from the store by name: its host, its attachments and what they give, its partner, its original and its front face. */
import { useMemo } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import type { ObjectView } from '@/protocol/generated'
import { attachedWords, attachesOf } from './attachMarks'
import type { Links } from './cardModel'
import { useSeatNamer } from './names'
import { useFrontFaceName } from './printed'
import { useDoors } from './useDoors'

/** The name of the creature a creature is paired with: hidden when the viewer may not look at it, or not in the store. */
function partnerName(partner: ObjectView | undefined): string {
  return partner === undefined || partner.name === '' ? 'a face-down creature' : partner.name
}

/** The links of a card; a new object only when a name changes, so a memoised card redraws for nothing else. */
export function useLinks(object: ObjectView | undefined): Links {
  const namer = useSeatNamer()
  // An Aura on a permanent is "on" that permanent's name; a Curse on a player, on the player's.
  const objectHost = usePlay((state) =>
    object?.attached_to === undefined ? undefined : state.objectsById[object.attached_to]?.name,
  )
  const host =
    objectHost ?? (object?.attached_player === undefined ? undefined : namer(object.attached_player))
  const attachments = usePlay(
    useShallow((state) =>
      (object?.attachments ?? []).flatMap((at) => {
        const attachment = state.objectsById[at]
        return attachment === undefined ? [] : [attachment]
      }),
    ),
  )
  const id = object?.id
  const controller = object?.controller
  const attaches = useMemo(
    () => (id === undefined || controller === undefined ? [] : attachesOf({ id, controller }, attachments)),
    [id, controller, attachments],
  )
  const spoken = useMemo(() => attachedWords(attaches, namer), [attaches, namer])
  const copyOf = usePlay((state) =>
    object?.copy?.of === undefined ? undefined : state.objectsById[object.copy.of]?.name,
  )
  const frontFace = useFrontFaceName(object)
  const doors = useDoors(object)
  const partner = usePlay((state) =>
    object?.paired_with === undefined ? undefined : partnerName(state.objectsById[object.paired_with]),
  )
  return useMemo(
    () => ({ host, attaches, spoken, copyOf, frontFace, partner, doors }),
    [host, attaches, spoken, copyOf, frontFace, partner, doors],
  )
}
