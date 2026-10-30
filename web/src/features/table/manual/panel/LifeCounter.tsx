import { useId, useRef } from 'react'
import type { KeyboardEvent } from 'react'
import { HeartIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { FeedbackLine } from './FeedbackLine'
import { LIFE_STEPS, lifeWords, signed, useLifeRun } from './lifeRun'
import type { PanelInput } from './commands'

/** The keys of the counter's buttons and the change each one makes; Page Up and Page Down move by five. */
const KEY_STEPS: Readonly<Record<string, number>> = { ArrowUp: 1, ArrowDown: -1, PageUp: 5, PageDown: -5 }

const KEYS_HINT =
  'Arrow Up and Arrow Down change life by 1, Page Up and Page Down by 5; Escape forgets what is not sent.'

/**
 * The seat's life as a life counter shows it: the total in large figures with -5, -1, +1 and +5 under it. A run of
 * presses is one `Life` action sent after a quiet moment, shown as "not sent yet" with Undo until then; Undo also takes
 * back the last change for a few seconds after the server took it. The arrow and page keys work from any button.
 */
export function LifeCounter({ input }: { input: PanelInput }) {
  const hint = useId()
  const run = useLifeRun({ seat: input.seat, life: input.life, off: input.off })
  const inert = input.off !== undefined
  const buttons = useRef<HTMLDivElement>(null)
  // The Undo link goes away when pressed, so the focus goes back to the buttons.
  const takeBack = (): void => {
    run.takeBack()
    buttons.current?.querySelector('button')?.focus({ preventScroll: true })
  }
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>): void => {
    const step = KEY_STEPS[event.key]
    if (step !== undefined) {
      event.preventDefault()
      event.stopPropagation()
      run.tap(step)
    } else if (event.key === 'Escape' && run.undo?.kind === 'drop') {
      event.preventDefault()
      event.stopPropagation()
      run.takeBack()
    }
  }
  return (
    <div data-slot="life-counter" className="grid gap-1.5 rounded-md bg-muted/50 px-2 py-2">
      <p className="flex items-center justify-between gap-2">
        <span className="flex min-w-0 items-center gap-1.5 text-sm font-medium">
          <HeartIcon aria-hidden="true" className="size-4 shrink-0 text-muted-foreground" />
          <span className="truncate">{input.name}</span>
        </span>
        <span data-pending={run.pending !== 0 || undefined}>
          <span data-slot="life-total" className="text-4xl leading-tight font-semibold tabular-nums">
            {run.total}
          </span>{' '}
          <span className="text-xs text-muted-foreground">life</span>
        </span>
      </p>
      <span id={hint} className="sr-only">
        {KEYS_HINT}
      </span>
      <div
        ref={buttons}
        role="group"
        aria-label="Change life"
        aria-describedby={hint}
        onKeyDown={onKeyDown}
        className="grid grid-cols-4 gap-1"
      >
        {LIFE_STEPS.map((step) => (
          <Button
            key={step}
            variant="outline"
            aria-label={lifeWords(step)}
            aria-disabled={inert || undefined}
            onClick={() => run.tap(step)}
            className="h-10 px-0 text-base tabular-nums pointer-coarse:h-11"
          >
            {signed(step)}
          </Button>
        ))}
      </div>
      <div data-slot="life-status" className="flex min-h-6 flex-wrap items-center justify-between gap-x-2">
        {run.pending !== 0 ? (
          <p data-slot="life-pending" className="text-xs text-muted-foreground tabular-nums">
            Not sent yet: {signed(run.pending)} ({input.life} → {run.total} life)
          </p>
        ) : (
          <FeedbackLine feedback={run.feedback} />
        )}
        {run.undo === null ? null : (
          <Button
            variant="link"
            size="sm"
            data-slot="life-undo"
            onClick={takeBack}
            className="px-1 text-xs tabular-nums"
          >
            Undo {signed(run.undo.change)}
          </Button>
        )}
      </div>
      <p aria-live="polite" className="sr-only">
        {input.name}: {run.total} life{run.pending !== 0 ? ', not sent yet' : ''}
      </p>
    </div>
  )
}
