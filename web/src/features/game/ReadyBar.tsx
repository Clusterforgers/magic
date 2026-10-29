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
      className="sticky bottom-0 z-10 -mx-4 grid gap-2 border-t bg-background/95 px-4 py-3 backdrop-blur sm:-mx-6 sm:px-6 min-[1000px]:static min-[1000px]:mx-0 min-[1000px]:rounded-xl min-[1000px]:border min-[1000px]:bg-card min-[1000px]:px-4"
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
