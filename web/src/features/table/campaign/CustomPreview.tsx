/**
 * The card preview of a custom card (a boss's innate, phase, modifier or relic, a hero's relic): its framed card, with
 * its name, type and rules, where a real card shows its picture; beside it what it is to the fight and, for a phase,
 * the boss's phases in order, each a step the preview moves to. It docks, fits, keeps clear and closes as the preview
 * of any card does.
 */
import { X } from 'lucide-react'
import { useEffect, useRef } from 'react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { CustomCardFrame } from '@/features/delves/CustomCardFrame'
import { useKeepClear } from '../keepClear'
import { usePreviewStore } from '../preview'
import type { CustomShown, Dock, PreviewTarget } from '../preview'
import { usePreviewFit } from '../previewReserve'
import { usePreviewDismiss } from '../usePreviewDismiss'
import { customPreviewId } from './customCards'

// Whole class names, so Tailwind can see them.
const DOCK: Record<Dock, string> = {
  right: 'right-3',
  left: 'left-3',
  top: 'inset-x-2',
  bottom: 'inset-x-2',
}

/** The boss's phases in order, each a button that shows that phase; the one on show is marked. */
function PhaseSteps({ shown, target }: { shown: CustomShown; target: PreviewTarget }) {
  const store = usePreviewStore()
  const phases = shown.phases ?? []
  if (phases.length < 2) return null
  return (
    <>
      <p
        aria-hidden="true"
        className="mt-1 text-xs font-semibold tracking-wide text-muted-foreground uppercase"
      >
        Phases
      </p>
      <ol aria-label="Phases" data-slot="preview-phases" className="grid gap-1">
        {phases.map((phase) => {
          const current = phase.card.name === shown.card.name
          const id = customPreviewId(phase.card.name)
          return (
            <li key={phase.card.name}>
              <button
                type="button"
                data-slot="preview-phase"
                data-object-id={id}
                aria-current={current || undefined}
                onClick={() => {
                  if (!current) store.pin(id, target.anchor, undefined, phase)
                }}
                className={cn(
                  'grid w-full min-h-11 content-center rounded-md border px-2 py-1 text-left outline-none focus-visible:ring-2 focus-visible:ring-ring',
                  current ? 'border-boss bg-boss/10' : 'border-border hover:bg-muted',
                )}
              >
                <span className="text-xs font-medium text-boss">{phase.kicker}</span>
                <span className="font-medium">{phase.card.name}</span>
              </button>
            </li>
          )
        })}
      </ol>
    </>
  )
}

/** The preview panel of the custom card `shown`, placed as `target` asks. */
export function CustomPreviewBody({ target, shown }: { target: PreviewTarget; shown: CustomShown }) {
  const store = usePreviewStore()
  const clear = useKeepClear()
  const fit = usePreviewFit(target, clear)
  const panel = useRef<HTMLDivElement>(null)
  usePreviewDismiss(target, panel, store)
  const { pinned } = target
  useEffect(() => {
    const close = (event: KeyboardEvent): void => {
      if (event.key !== 'Escape') return
      if (pinned) event.preventDefault()
      store.dismiss()
    }
    document.addEventListener('keydown', close)
    return () => document.removeEventListener('keydown', close)
  }, [store, pinned])
  const { dock, beside } = fit
  const { card } = shown
  return (
    <div
      role="group"
      aria-label="Card preview"
      ref={panel}
      data-testid="card-preview"
      data-custom={card.kind}
      data-dock={dock}
      data-picture={fit.picture}
      data-pinned={pinned || undefined}
      style={{
        top: fit.top ?? undefined,
        bottom: fit.bottom ?? undefined,
        width: fit.width ?? undefined,
        maxHeight: fit.maxHeight,
      }}
      className={cn(
        'fixed z-[60] overflow-x-hidden overflow-y-auto rounded-xl bg-popover p-3 text-popover-foreground shadow-lg ring-1 ring-foreground/15',
        DOCK[dock],
        beside ? 'flex gap-3' : null,
        pinned ? null : 'pointer-events-none',
      )}
    >
      <div
        data-slot="preview-picture"
        className={beside ? 'shrink-0' : 'mx-auto w-fit max-w-full'}
        style={{ width: fit.picture }}
      >
        <CustomCardFrame card={card} className="sm:w-full" />
      </div>
      <div
        data-slot="preview-details"
        className={cn(
          'grid grid-cols-[minmax(0,1fr)] content-start gap-1 text-sm [overflow-wrap:anywhere]',
          beside ? 'min-w-0 flex-1' : 'mt-3',
        )}
      >
        {/* The frame carries the name, type and rules; the lines here say what the card is to this fight. */}
        {shown.kicker === undefined ? null : (
          <p
            data-slot="preview-kicker"
            className="w-fit rounded-md bg-boss/10 px-2 py-0.5 text-xs font-medium text-foreground"
          >
            {shown.kicker}
          </p>
        )}
        <PhaseSteps shown={shown} target={target} />
        {shown.place === undefined ? null : <p className="text-muted-foreground">{shown.place}.</p>}
        {pinned ? (
          <div className="sticky -bottom-3 -mb-3 flex flex-wrap gap-2 bg-popover pt-1 pb-3">
            <Button variant="outline" size="sm" onClick={() => store.dismiss()}>
              <X aria-hidden="true" />
              Close preview
            </Button>
          </div>
        ) : null}
      </div>
    </div>
  )
}
