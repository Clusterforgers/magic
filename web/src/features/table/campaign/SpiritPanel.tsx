/**
 * The Spirit panel: a fallen hero's haunt actions as big buttons, each with its ectoplasm price, a plain sentence and
 * its effect in short, the ectoplasm held as pips, and why an action cannot be taken now. A click activates the action
 * through the priority answer; everything else (targets, the stack) is the usual prompt.
 */
import { GhostIcon } from 'lucide-react'
import type { AbilityRef, ObjectView, PlayerView, PriorityOptions } from '@/protocol/generated'
import { cn } from '@/lib/utils'
import { ectoplasmOf, hauntOptions } from './haunt'

/** The ectoplasm held as pips, a filled one each, and the number. */
export function EctoplasmPips({ count }: { count: number }) {
  const shown = Math.min(count, 6)
  return (
    <span data-slot="ectoplasm" className="inline-flex items-center gap-1 text-xs font-semibold text-spirit">
      <span aria-hidden="true" className="inline-flex gap-0.5">
        {Array.from({ length: shown }, (_, at) => (
          <span key={at} className="size-2 rounded-full bg-spirit" />
        ))}
      </span>
      <span>{count} ectoplasm</span>
    </span>
  )
}

export function SpiritPanel({
  player,
  emblem,
  options,
  onActivate,
}: {
  player: PlayerView | undefined
  emblem: ObjectView | undefined
  options: PriorityOptions | null
  onActivate: (ability: AbilityRef) => void
}) {
  const ectoplasm = ectoplasmOf(player)
  const actions = hauntOptions(player, emblem, options)
  return (
    <section
      aria-label="Haunt actions"
      data-slot="spirit-panel"
      className="grid gap-2 rounded-xl border border-dashed border-spirit bg-surface p-3"
    >
      <header className="flex flex-wrap items-center justify-between gap-2">
        <h2 className="inline-flex items-center gap-2 text-sm font-semibold text-spirit">
          <GhostIcon aria-hidden="true" className="size-4" />
          You are a Restless Spirit
        </h2>
        <EctoplasmPips count={ectoplasm} />
      </header>
      <p className="text-xs text-muted-foreground">
        Enemies can&apos;t attack or target you. You gain 1 ectoplasm each of your turns; spend it to help.
      </p>
      <ul className="grid grid-cols-[repeat(auto-fill,minmax(11rem,1fr))] gap-2">
        {actions.map((action) => {
          const ready = action.ready
          return (
            <li key={action.index}>
              <button
                type="button"
                data-slot="haunt-action"
                data-ready={ready !== undefined || undefined}
                aria-disabled={ready === undefined}
                title={action.plain}
                aria-describedby={`haunt-${action.index}`}
                onClick={() => {
                  if (ready !== undefined) onActivate(ready)
                }}
                className={cn(
                  'grid min-h-11 w-full gap-1 rounded-lg border p-2 text-start transition-colors duration-fast',
                  ready === undefined
                    ? 'cursor-not-allowed border-border text-muted-foreground'
                    : 'border-spirit bg-spirit/10 text-foreground hover:bg-spirit/20',
                )}
              >
                <span className="flex items-center justify-between gap-2">
                  <span className="text-base font-semibold">{action.name}</span>
                  <span className="text-xs font-medium text-spirit">{action.ectoplasm} ecto</span>
                </span>
                <span id={`haunt-${action.index}`} className="text-xs">
                  {action.plain}
                </span>
                <span className="text-xs font-medium">
                  {ready === undefined ? action.why : action.preview}
                </span>
              </button>
            </li>
          )
        })}
      </ul>
    </section>
  )
}
