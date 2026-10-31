/** The doors of a Room permanent, named by the faces of its printed card. */
import { useMemo } from 'react'
import type { ObjectView } from '@/protocol/generated'
import { useDoorFaces } from './printed'
import { doorsNamed } from './rooms'
import type { Doors } from './rooms'

/** The doors of a Room permanent, named by the card's printed faces once they are known; a new value only when a door or a name changes. */
export function useDoors(object: ObjectView | undefined): Doors | undefined {
  const faces = useDoorFaces(object)
  const state = object?.permanent?.doors
  const name = object?.name ?? ''
  const [open, shut] = [state?.[0], state?.[1]]
  const [first, second] = [faces?.[0], faces?.[1]]
  return useMemo(
    () =>
      open === undefined || shut === undefined ? undefined : doorsNamed(name, [open, shut], first, second),
    [name, open, shut, first, second],
  )
}
