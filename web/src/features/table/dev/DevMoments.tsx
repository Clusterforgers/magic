import { useEffect } from 'react'
import { useGame } from '@/features/game/gameContext'
import { applyBatchToStore } from './applyBatch'
import { FIRST_MOVE_MS } from './DevFxControls'
import { momentsBatch } from './momentScene'

/** Plays the reveal (of `count` cards) and the die roll once, `FIRST_MOVE_MS` after the page has loaded (`?moments=1`), so the moments can be looked at. */
export function DevMoments({ on, count }: { on: boolean; count?: number | undefined }) {
  const { session } = useGame()
  useEffect(() => {
    if (!on) return undefined
    const timer = window.setTimeout(() => {
      const batch = momentsBatch(session.store.getState(), count)
      if (batch !== null) applyBatchToStore(session.store, batch)
    }, FIRST_MOVE_MS)
    return () => window.clearTimeout(timer)
  }, [on, session, count])
  return null
}
