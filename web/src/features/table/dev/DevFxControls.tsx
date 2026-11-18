import { Sparkles } from 'lucide-react'
import { useEffect, useId, useLayoutEffect, useState } from 'react'
import { createPortal } from 'react-dom'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { useGame } from '@/features/game/gameContext'
import { DEV_FX } from '../devSearch'
import type { DevFx } from '../devSearch'
import { playDevFx } from './fxScenarios'

/** How long after the page has loaded `?fx=` plays its move, so the table has been drawn and watched by then. */
export const FIRST_MOVE_MS = 600

/**
 * One button, "Effects", that opens a bar of buttons which play a move on the fixture table (an attack, a block, damage,
 * a death, a life change, an entry) so its effect can be looked at. It stands in the top bar's tool slot (`bar-tools`),
 * in the bar's flow, so it covers no control at any width (an icon on a phone), and its moves drop below the bar; a
 * page without the slot keeps it in the bottom left corner. `data-dev-tool` lets the layout gate hide it; `?bar=1`
 * opens it. It also plays the move `?fx=` names once, shortly after the page has loaded.
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
  // The top bar's tool slot, once the table has drawn it; a page without one (game over) keeps the fixed corner.
  const [slot, setSlot] = useState<HTMLElement | null>(null)
  useLayoutEffect(() => {
    setSlot(document.querySelector<HTMLElement>('[data-slot="bar-tools"]'))
  }, [])
  const group = (
    <div
      role="group"
      aria-label="Table effects (development)"
      data-dev-tool=""
      data-open={open || undefined}
      data-in-bar={slot !== null || undefined}
      className={cn(
        'flex items-center gap-1 text-xs',
        slot === null &&
          'fixed bottom-1.5 left-1.5 z-50 max-w-[calc(100vw-3rem)] flex-wrap rounded-lg border bg-popover p-1 text-popover-foreground shadow-md',
      )}
    >
      <Button
        type="button"
        size="xs"
        variant={open ? 'secondary' : 'outline'}
        aria-expanded={open}
        aria-controls={open ? bar : undefined}
        aria-label="Effects"
        onClick={() => setOpen((now) => !now)}
      >
        <Sparkles aria-hidden="true" className="sm:hidden" />
        <span className="max-sm:hidden">Effects</span>
      </Button>
      {open ? (
        <div
          id={bar}
          className={cn(
            'flex flex-wrap items-center gap-1',
            // In the bar the moves drop below it as a menu does, over the table and never over the bar.
            slot !== null &&
              'absolute top-full right-0 z-50 mt-1 w-max max-w-[min(36rem,calc(100vw-1rem))] justify-end rounded-lg max-sm:fixed max-sm:inset-x-2 max-sm:top-12 max-sm:mt-0 max-sm:w-auto max-sm:max-w-none border bg-popover p-1 text-popover-foreground shadow-md',
          )}
        >
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
  return slot === null ? group : createPortal(group, slot)
}
