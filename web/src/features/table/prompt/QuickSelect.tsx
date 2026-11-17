import { useId } from 'react'
import { Button } from '@/components/ui/button'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import type { QuickButton } from './quickSelect'

/** Props of {@link QuickSelect}. */
export interface QuickSelectProps<T> {
  buttons: readonly QuickButton<T>[]
  onSet(next: readonly T[]): void
}

/**
 * The row of quick selection buttons over a prompt's choices ([`quickButtons`](./quickSelect.ts)). A button that is off
 * stays focusable with `aria-disabled` and tells why in a tooltip and to a screen reader; each is 44 px high under a
 * coarse pointer.
 */
export function QuickSelect<T>({ buttons, onSet }: QuickSelectProps<T>) {
  const base = useId()
  if (buttons.length === 0) return null
  return (
    <div
      role="group"
      aria-label="Quick selection"
      data-slot="quick-select"
      className="flex flex-wrap gap-1.5"
    >
      {buttons.map((button) => {
        const off = button.reason !== undefined
        const reasonId = `${base}-${button.kind}`
        const press = (
          <Button
            size="sm"
            variant="outline"
            data-kind={button.kind}
            aria-disabled={off || undefined}
            aria-describedby={off ? reasonId : undefined}
            className="aria-disabled:opacity-50"
            onClick={() => {
              if (!off) onSet(button.next)
            }}
          >
            {button.label}
          </Button>
        )
        return off ? (
          <span key={button.kind} className="contents">
            <Tooltip>
              <TooltipTrigger render={press} />
              <TooltipContent side="top">{button.reason}</TooltipContent>
            </Tooltip>
            <span id={reasonId} className="sr-only">
              {button.reason}
            </span>
          </span>
        ) : (
          <span key={button.kind} className="contents">
            {press}
          </span>
        )
      })}
    </div>
  )
}
