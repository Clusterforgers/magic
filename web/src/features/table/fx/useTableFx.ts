import { useEffect, useMemo } from 'react'
import { useGame } from '@/features/game/gameContext'
import { watchBatches } from './batch'
import { createFx, systemMode } from './store'
import type { Fx, FxMode } from './store'

/**
 * Whether a table decorates when nothing says otherwise: everywhere except under vitest, where the layer stays off
 * so that tests see the table as the game says it and wait for no timer. A test that wants it passes `effects: true`.
 */
export const EFFECTS_BY_DEFAULT = import.meta.env.MODE !== 'test'

const OFF = (): FxMode => 'off'

/**
 * The decoration layer of the table that calls it. While `enabled` it watches the play store and plays the effects
 * of every batch the store applies, until the table goes; `slow` multiplies every duration (the dev route's
 * `?slow=`). The layer follows the system's reduced-motion setting at every batch.
 */
export function useTableFx(enabled: boolean, slow = 1): Fx {
  const { session } = useGame()
  const fx = useMemo(() => createFx({ slow, mode: enabled ? systemMode : OFF }), [enabled, slow])
  useEffect(() => {
    if (!enabled) return undefined
    const stop = watchBatches(session.store, fx)
    return () => {
      stop()
      fx.clear()
    }
  }, [enabled, session, fx])
  return fx
}
