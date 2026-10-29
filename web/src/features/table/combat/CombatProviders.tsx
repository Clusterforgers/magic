import { useContext, useLayoutEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { createDefenderStore, DefenderContext } from './defender'
import { createDrawnForecastStore, DrawnForecastContext } from './drawn'
import { useBlockPreview } from './forecast'
import { createPendingCombatStore, PendingCombatContext } from './pending'

/** Publishes the forecast the seam says to draw, for as long as the table is there. */
function DrawnForecast(): null {
  const forecast = useBlockPreview()
  const store = useContext(DrawnForecastContext)
  // Layout effects, so a card never paints once with the forecast that was drawn before.
  useLayoutEffect(() => {
    store.publish(forecast)
  }, [store, forecast])
  useLayoutEffect(() => () => store.publish(null), [store])
  return null
}

/**
 * What the combat marks of a table share: the choices a prompt has not sent (`pending.ts`), the forecast on screen
 * (`drawn.ts`) and the defender an attack is aimed at (`defender.ts`). `GameTable` wraps itself in this once, inside the game.
 */
export function CombatProviders({ children }: { children: ReactNode }) {
  const [pending] = useState(createPendingCombatStore)
  const [drawn] = useState(createDrawnForecastStore)
  const [defenders] = useState(createDefenderStore)
  return (
    <PendingCombatContext value={pending}>
      <DrawnForecastContext value={drawn}>
        <DefenderContext value={defenders}>
          <DrawnForecast />
          {children}
        </DefenderContext>
      </DrawnForecastContext>
    </PendingCombatContext>
  )
}
