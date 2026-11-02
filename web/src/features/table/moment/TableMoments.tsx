import { X } from 'lucide-react'
import { useEffect } from 'react'
import { useStore } from 'zustand'
import { Button } from '@/components/ui/button'
import { useReducedMotion } from '@/hooks/useReducedMotion'
import { useSeatNamer } from '../names'
import { PreviewCard } from '../PreviewCard'
import { useViewport } from '../useViewport'
import { DieFace } from './DieFace'
import { revealWidth } from './moments'
import type { RevealMoment, RollMoment } from './moments'
import { useMoments } from './store'

/** The title line of a moment: who did it and, after a dot, which spell or ability made them. */
function Heading({ words, why, onDismiss }: { words: string; why: string | null; onDismiss(): void }) {
  return (
    <div className="flex items-start justify-between gap-3">
      <p data-slot="moment-title" className="min-w-0 text-sm">
        <span className="font-semibold">{words}</span>
        {why === null ? null : <span className="text-muted-foreground"> · {why}</span>}
      </p>
      <Button
        type="button"
        size="icon-sm"
        variant="ghost"
        aria-label="Dismiss"
        className="-mt-1 -mr-1 shrink-0"
        onClick={onDismiss}
      >
        <X aria-hidden="true" />
      </Button>
    </div>
  )
}

/** Cards a player revealed, large, with their pictures, who revealed them and why. */
function Reveal({
  moment,
  crowded,
  onDismiss,
}: {
  moment: RevealMoment
  crowded: boolean
  onDismiss(): void
}) {
  const seat = useSeatNamer()
  const viewport = useViewport()
  const width = revealWidth(moment.cards.length, viewport, crowded)
  const who = seat(moment.player)
  return (
    <section
      data-slot="moment"
      data-kind="reveal"
      aria-label={`${who} reveals`}
      className="pointer-events-auto max-h-[calc(100dvh-5rem)] max-w-full overflow-y-auto rounded-xl bg-popover p-3 text-popover-foreground shadow-lg ring-1 ring-foreground/15"
    >
      <Heading words={`${who} reveals`} why={moment.why} onDismiss={onDismiss} />
      <ul aria-label="Revealed cards" className="mt-2 flex max-w-full items-start gap-3 overflow-x-auto pb-1">
        {moment.cards.map((card) => (
          <li key={card.id} data-slot="moment-card" className="shrink-0">
            <PreviewCard object={card} role={null} size="large" width={width} revealed />
          </li>
        ))}
      </ul>
    </section>
  )
}

/** A die rolled by a player: it tumbles and lands on the result, or shows the result at once under reduced motion. */
function Roll({ moment, still, onDismiss }: { moment: RollMoment; still: boolean; onDismiss(): void }) {
  const seat = useSeatNamer()
  const who = seat(moment.player)
  return (
    <section
      data-slot="moment"
      data-kind="roll"
      aria-label={`${who} rolls a d${moment.sides}`}
      className="pointer-events-auto w-fit max-w-full rounded-xl bg-popover p-3 text-popover-foreground shadow-lg ring-1 ring-foreground/15"
    >
      <Heading words={`${who} rolls a d${moment.sides}`} why={moment.why} onDismiss={onDismiss} />
      <div className="mt-1 grid place-items-center">
        <DieFace sides={moment.sides} result={moment.result} rollKey={moment.key} still={still} />
      </div>
    </section>
  )
}

/**
 * What every seat is shown for a moment: the cards a player revealed, large, and a die a player rolled, landing on its
 * result. They stand at the top of the table for `momentMs`, never take the pointer outside their own panel (the table
 * stays playable under them), and go with a click on Dismiss or Escape. Under reduced motion nothing moves. The log
 * says the same in words, so this is a way to see it, not the only one.
 */
export function TableMoments() {
  const moments = useMoments()
  const items = useStore(moments.store, (state) => state.items)
  const reduced = useReducedMotion()
  const open = items.length > 0
  useEffect(() => {
    if (!open) return undefined
    const close = (event: KeyboardEvent): void => {
      if (event.key === 'Escape') moments.clear()
    }
    document.addEventListener('keydown', close)
    return () => document.removeEventListener('keydown', close)
  }, [open, moments])
  if (!open) return null
  return (
    <div
      data-slot="moments"
      className="pointer-events-none fixed inset-x-2 top-14 z-[55] flex flex-wrap items-start justify-center gap-3"
    >
      {items.map((moment) =>
        moment.kind === 'reveal' ? (
          <Reveal
            key={moment.key}
            moment={moment}
            crowded={items.length > 1}
            onDismiss={() => moments.dismiss(moment.key)}
          />
        ) : (
          <Roll
            key={moment.key}
            moment={moment}
            still={reduced}
            onDismiss={() => moments.dismiss(moment.key)}
          />
        ),
      )}
    </div>
  )
}
