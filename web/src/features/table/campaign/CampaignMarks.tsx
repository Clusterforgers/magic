/**
 * A campaign fight's marks on a pod and a tile: the boss's crown chip and phase track, and the Spirit mark of a fallen
 * hero. They read the seat's public `boss` and `spirit` fields only.
 */
import { CrownIcon, GhostIcon } from 'lucide-react'
import type { BossView } from '@/protocol/generated'
import { cn } from '@/lib/utils'
import { phaseSteps } from './bossPhase'

/** The crown chip beside a boss's name. */
export function BossChip({ compact = false }: { compact?: boolean }) {
  return (
    <span
      data-slot="boss-chip"
      className="inline-flex shrink-0 items-center gap-1 rounded-full bg-boss px-1.5 text-xs leading-5 font-semibold text-on-boss"
    >
      <CrownIcon aria-hidden="true" className="size-3" />
      <span className={cn(compact && 'sr-only')}>Boss</span>
    </span>
  )
}

/**
 * The boss's phases as a track: one segment per phase, the begun ones filled, the current one ringed and named
 * ("Phase II · The Nest Stirs"); the next ones are hollow. A list for a screen reader, each step saying its state.
 */
export function BossTrack({ boss, named = true }: { boss: BossView; named?: boolean }) {
  const steps = phaseSteps(boss)
  const current = steps.find((step) => step.state === 'current')
  return (
    <div data-slot="boss-track" className="flex min-w-0 items-center gap-2">
      <ol aria-label="Boss phases" className="flex shrink-0 items-center gap-1">
        {steps.map((step) => (
          <li
            key={step.label}
            data-state={step.state}
            aria-current={step.state === 'current' ? 'step' : undefined}
            className={cn(
              'h-2 w-4 rounded-full border border-boss',
              step.state === 'next' ? 'bg-transparent' : 'bg-boss',
              step.state === 'current' && 'ring-1 ring-boss ring-offset-1 ring-offset-card',
            )}
          >
            <span className="sr-only">
              {step.label}
              {step.title === '' ? '' : `: ${step.title}`}
              {step.state === 'done' ? ', passed' : step.state === 'current' ? ', now' : ', to come'}
            </span>
          </li>
        ))}
      </ol>
      {named && current ? (
        <span aria-hidden="true" className="truncate text-xs font-medium text-boss">
          {current.label}
          {current.title === '' ? '' : ` · ${current.title}`}
        </span>
      ) : null}
    </div>
  )
}

/** The mark beside a fallen hero's name: a ghost and "Spirit". */
export function SpiritMark({ compact = false }: { compact?: boolean }) {
  return (
    <span
      data-slot="spirit-mark"
      className="inline-flex shrink-0 items-center gap-1 rounded-full border border-spirit px-1.5 text-xs leading-4 font-semibold text-spirit"
    >
      <GhostIcon aria-hidden="true" className="size-3" />
      <span className={cn(compact && 'sr-only')}>Spirit</span>
    </span>
  )
}
