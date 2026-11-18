/**
 * A reworked boss's phase pill at the head of its bar: "Phase 1 in 3 rounds" until the phase begins, then the phase's
 * name, with the countdown to the next beside it where there is room. The pill keeps one width whatever it says, so
 * the chips after it never move; a press lists both phases with their rounds and rules.
 */
import { FlameIcon, HourglassIcon } from 'lucide-react'
import { Popover, PopoverContent, PopoverTitle, PopoverTrigger } from '@/components/ui/popover'
import { cn } from '@/lib/utils'
import type { BossView } from '@/protocol/generated'
import { phaseAt, phaseCountdown, phaseLabel } from './reworkedBoss'

/** The phase pill of the reworked boss `boss`; nothing for a boss without phases by round. */
export function PhaseCountdown({ boss }: { boss: BossView }) {
  const countdown = phaseCountdown(boss)
  if (countdown === null) return null
  const Icon = countdown.begun ? FlameIcon : HourglassIcon
  const phases = boss.round_phases ?? []
  return (
    <Popover>
      <PopoverTrigger
        openOnHover
        delay={200}
        data-slot="phase-countdown"
        data-begun={countdown.begun || undefined}
        aria-label={countdown.title}
        className={cn(
          'flex h-7 w-44 shrink-0 items-center gap-1 rounded-full border px-2 text-left text-xs font-semibold outline-none hover:brightness-110 focus-visible:ring-2 focus-visible:ring-ring sm:w-64',
          countdown.begun ? 'border-boss bg-boss text-on-boss' : 'border-boss/60 bg-boss/10 text-foreground',
        )}
      >
        <Icon
          aria-hidden="true"
          className={cn('size-3.5 shrink-0', countdown.begun ? 'text-on-boss' : 'text-boss')}
        />
        <span aria-hidden="true" className="min-w-0 truncate">
          {countdown.label}
        </span>
        {countdown.next === undefined ? null : (
          <span aria-hidden="true" className="hidden min-w-0 truncate font-medium opacity-80 sm:inline">
            · {countdown.next}
          </span>
        )}
      </PopoverTrigger>
      <PopoverContent align="start" className="w-80 max-w-[calc(100vw-2rem)] gap-2 p-3 text-sm">
        <PopoverTitle>Phases</PopoverTitle>
        <ol className="grid gap-2">
          {phases.map((phase, at) => (
            <li key={phase.name} data-slot="phase-countdown-step" data-begun={phase.begun || undefined}>
              <p className="text-xs font-medium text-boss">
                {phase.begun ? `${phaseLabel(at)} · in force` : phaseAt(at, phase)}
              </p>
              <p className="font-semibold">{phase.name}</p>
              <p className="text-muted-foreground">{phase.text}</p>
            </li>
          ))}
        </ol>
      </PopoverContent>
    </Popover>
  )
}
