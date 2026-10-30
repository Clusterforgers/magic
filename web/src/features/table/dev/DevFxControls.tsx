import { useEffect, useId, useState } from 'react'
import { Button } from '@/components/ui/button'
import { useGame } from '@/features/game/gameContext'
import { DEV_FX } from '../devSearch'
import type { DevFx } from '../devSearch'
import { playDevFx } from './fxScenarios'

/** How long after the page has loaded `?fx=` plays its move, so the table has been drawn and watched by then. */
export const FIRST_MOVE_MS = 600

/**
 * One button, "Effects", that opens a bar of buttons which play a move on the fixture table (an attack, a block, damage,
 * a death, a life change, an entry) so its effect can be looked at. Folded it covers a corner of the top bar only, so a
 * screenshot shows the table; `?bar=1` opens it. It also plays the move `?fx=` names once, shortly after the page has
 * loaded.
 */
export function DevFxControls({
  initial,
  open: opened = false,
}: {
  initial?: DevFx | undefined
  /** The bar starts open. */
  open?: boolean | undefined
}) {
  const { session } = useGame()
  const [open, setOpen] = useState(opened)
  const bar = useId()
  useEffect(() => {
    if (initial === undefined) return undefined
    const timer = window.setTimeout(() => playDevFx(session.store, initial), FIRST_MOVE_MS)
    return () => window.clearTimeout(timer)
  }, [initial, session])
  return (
    <div
      role="group"
      aria-label="Table effects (development)"
      data-open={open || undefined}
      className="fixed top-1.5 right-14 z-50 flex max-w-[calc(100vw-4rem)] lg:right-[26rem] lg:max-w-[calc(100vw-27rem)] flex-wrap items-center justify-end gap-1 rounded-lg border bg-popover p-1 text-xs text-popover-foreground shadow-md"
    >
      <Button
        type="button"
        size="xs"
        variant={open ? 'secondary' : 'outline'}
        aria-expanded={open}
        aria-controls={open ? bar : undefined}
        onClick={() => setOpen((now) => !now)}
      >
        Effects
      </Button>
      {open ? (
        <div id={bar} className="flex flex-wrap items-center gap-1">
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
      ) : null}
    </div>
  )
}
