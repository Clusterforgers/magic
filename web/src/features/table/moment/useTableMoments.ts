import { useEffect, useMemo } from 'react'
import { useGame } from '@/features/game/gameContext'
import { createMoments, watchMoments } from './store'
import type { Moments } from './store'

/**
 * The moments of the table that calls it: while `enabled` it watches the play store and shows the reveals and dice of
 * every batch the store applies, until the table goes. Off, nothing is shown (the tests of other things see the table as
 * the game says it).
 */
export function useTableMoments(enabled: boolean): Moments {
  const { session } = useGame()
  const moments = useMemo(() => createMoments(), [])
  useEffect(() => {
    if (!enabled) return undefined
    const stop = watchMoments(session.store, moments)
    return () => {
      stop()
      moments.clear()
    }
  }, [enabled, session, moments])
  return moments
}
