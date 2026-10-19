import { cn } from '@/lib/utils'
import type { Phase } from './phase'

/**
 * The phase's icon and name on its colour, the left end of the turn badge: the accent filled on the viewer's own turn,
 * the soft tint with an accent edge on another seat's. The colour is never the only sign, since the icon and the name
 * sit in it. It repeats what the badge's words say about the step, so a screen reader skips it.
 */
export function PhaseTag({ phase, yours }: { phase: Phase; yours: boolean }) {
  const Icon = phase.icon
  return (
    <span
      aria-hidden="true"
      data-slot="phase-tag"
      data-phase-id={phase.id}
      data-themed=""
      className={cn(
        'inline-flex h-7 shrink-0 items-center gap-1.5 rounded-l-full border-2 border-r-0 border-phase pr-2 pl-2.5 text-sm font-semibold whitespace-nowrap',
        yours ? 'bg-phase text-on-phase' : 'bg-phase-soft text-foreground',
      )}
    >
      <Icon data-themed="" className={cn('size-4 shrink-0', yours ? 'text-on-phase' : 'text-phase')} />
      {phase.name}
    </span>
  )
}
