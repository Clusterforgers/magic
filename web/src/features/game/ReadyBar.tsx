import type { MouseEvent, ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import { readyState } from './lobbyModel'
import type { ReadyFacts } from './lobbyModel'

interface ReadyBarProps extends ReadyFacts {
  onToggle(event: MouseEvent<HTMLButtonElement>): void
  /** Alerts under the button: a refused press, a lobby that ended. */
  children?: ReactNode
}

/**
 * The seat's Ready button with the words that say why it is off or what it does, always in the window: stuck to the
 * bottom of the page on a phone and under the deck on a wide page, so the player never scrolls to find it.
 */
export function ReadyBar({ onToggle, children, ...facts }: ReadyBarProps) {
  const state = readyState(facts)
  return (
    <div
      data-slot="ready-bar"
      // The page's gutter bleed stays below 1000 px: `sm:` sorts after an arbitrary `min-[1000px]:` and would win.
      className="grid gap-2 px-4 py-3 max-[999px]:sticky max-[999px]:bottom-0 max-[999px]:z-10 max-[999px]:-mx-4 max-[999px]:border-t max-[999px]:bg-background max-[999px]:sm:-mx-6 max-[999px]:sm:px-6 min-[1000px]:rounded-xl min-[1000px]:border min-[1000px]:bg-card"
    >
      <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
        <Button
          type="button"
          size="lg"
          variant={facts.ready ? 'outline' : 'default'}
          disabled={state.disabled}
          aria-describedby="ready-reason"
          className="min-w-32"
          onClick={onToggle}
        >
          {state.label}
        </Button>
        <p
          id="ready-reason"
          data-slot="ready-hint"
          className="min-w-0 flex-1 basis-56 text-sm text-muted-foreground [overflow-wrap:anywhere]"
        >
          {state.hint}
        </p>
      </div>
      {children}
    </div>
  )
}
