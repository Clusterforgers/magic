import { useEffect } from 'react'
import { Button } from '@/components/ui/button'
import { useGame } from '@/features/game/gameContext'
import { DEV_FX } from '../devSearch'
import type { DevFx } from '../devSearch'
import { playDevFx } from './fxScenarios'

/** How long after the page has loaded `?fx=` plays its move, so the table has been drawn and watched by then. */
export const FIRST_MOVE_MS = 600

/**
 * Buttons that play a move on the fixture table (an attack, a block, damage, a death, a life change, an entry) so
 * its effect can be looked at, and the move `?fx=` names once, shortly after the page has loaded.
 */
export function DevFxControls({ initial }: { initial?: DevFx | undefined }) {
  const { session } = useGame()
  useEffect(() => {
    if (initial === undefined) return undefined
    const timer = window.setTimeout(() => playDevFx(session.store, initial), FIRST_MOVE_MS)
    return () => window.clearTimeout(timer)
  }, [initial, session])
  return (
    <div
      role="group"
      aria-label="Table effects (development)"
      className="fixed bottom-16 left-2 z-50 flex flex-wrap items-center gap-1 rounded-lg border bg-popover p-1.5 text-xs text-popover-foreground shadow-md lg:top-1.5 lg:bottom-auto lg:left-1/2 lg:-translate-x-1/2"
    >
      <span className="px-1 font-medium">Effects</span>
      {DEV_FX.map((kind) => (
        <Button
          key={kind}
          type="button"
          size="xs"
          variant="outline"
          onClick={() => playDevFx(session.store, kind)}
        >
          {kind}
        </Button>
      ))}
    </div>
  )
}
