import { useCallback, useMemo, useSyncExternalStore } from 'react'
import type { ObjectId } from '@/protocol/generated'
import { useMarksStore } from './prompt/marks'

const NONE: ReadonlySet<ObjectId> = new Set()

/**
 * The cards among `ids` that the active prompt needs: marked legal or selected (a dimmed card is only in the way).
 * Redraws when that set changes and not for any other change of a mark.
 */
export function useMarkedIds(ids: readonly ObjectId[]): ReadonlySet<ObjectId> {
  const store = useMarksStore()
  const subscribe = useCallback(
    (listener: () => void) => {
      const stops = ids.map((id) => store.subscribe(id, listener))
      return () => {
        for (const stop of stops) stop()
      }
    },
    [store, ids],
  )
  // A string, so an unchanged set reads as the same snapshot.
  const snapshot = useSyncExternalStore(subscribe, () =>
    ids
      .filter((id) => {
        const mark = store.markOf(id)
        return mark === 'legal' || mark === 'selected'
      })
      .join(','),
  )
  return useMemo(() => (snapshot === '' ? NONE : new Set(snapshot.split(',').map(Number))), [snapshot])
}
