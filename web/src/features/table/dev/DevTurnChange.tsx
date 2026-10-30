import { useEffect } from 'react'
import { useGame } from '@/features/game/gameContext'
import { applyBatchToStore } from './applyBatch'
import { FIRST_MOVE_MS } from './DevFxControls'
import { turnChangeBatch } from './turnScene'

/** Passes the turn to the other side once, `FIRST_MOVE_MS` after the page has loaded (`?newturn=1`), so the turn cues can be looked at. */
export function DevTurnChange({ on }: { on: boolean }) {
  const { session } = useGame()
  useEffect(() => {
    if (!on) return undefined
    const timer = window.setTimeout(() => {
      const batch = turnChangeBatch(session.store.getState())
      if (batch !== null) applyBatchToStore(session.store, batch)
    }, FIRST_MOVE_MS)
    return () => window.clearTimeout(timer)
  }, [on, session])
  return null
}
