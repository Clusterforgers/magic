import { useMemo } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import { useSeatNamer } from './names'
import { stackTopFirst } from './selectors'
import { stackEntries } from './stackTargets'
import type { StackEntry } from './stackTargets'
import type { TargetNamer } from './stackText'

/** The stack top first with every item's targets; the same array until the stack, an object or the log changes. */
export function useStackEntries(): readonly StackEntry[] {
  const ids = usePlay(useShallow(stackTopFirst))
  const objectsById = usePlay((state) => state.objectsById)
  const players = usePlay((state) => state.view?.players)
  const log = usePlay((state) => state.log)
  return useMemo(
    () => stackEntries({ ids, objectsById, players: players ?? [], log }),
    [ids, objectsById, players, log],
  )
}

/** How the table names a seat or a card. */
export function useTargetNamer(): TargetNamer {
  const seat = useSeatNamer()
  const objectsById = usePlay((state) => state.objectsById)
  return useMemo(() => ({ seat, card: (id) => objectsById[id]?.name || 'a card' }), [seat, objectsById])
}
