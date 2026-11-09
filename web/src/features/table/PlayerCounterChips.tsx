/** The chips of a seat's pod or tile for the counters on the player: rad, poison, energy, experience and the rest. */
import { Biohazard, Radiation, Sparkles, Ticket, Zap } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { PlayerView } from '@/protocol/generated'
import { DENSITY } from './PodParts'
import type { ChipDensity } from './PodParts'
import { playerCounterChips, playerCounterLabel } from './playerCounters'
import type { PlayerCounterIcon } from './playerCounters'

const ICON: Record<PlayerCounterIcon, LucideIcon> = {
  poison: Biohazard,
  rad: Radiation,
  energy: Zap,
  experience: Sparkles,
  ticket: Ticket,
}

/** One chip per kind of counter on the player, an icon (or a short label) and the count, each named in words; nothing without counters. */
export function PlayerCounterChips({
  owner,
  player,
  density = 'line',
}: {
  owner: string
  player: Pick<PlayerView, 'counters'>
  density?: ChipDensity
}) {
  const chips = playerCounterChips(player)
  if (chips.length === 0) return null
  return (
    <>
      {chips.map((chip) => {
        const label = playerCounterLabel(owner, chip)
        const Icon = chip.icon === null ? null : ICON[chip.icon]
        return (
          <span
            key={chip.key}
            role="img"
            aria-label={label}
            title={label}
            data-slot="player-counter"
            data-kind={chip.key}
            className={cn(DENSITY[density].chip, 'shrink-0 text-muted-foreground')}
          >
            {Icon === null ? (
              <span aria-hidden="true">{chip.short}</span>
            ) : (
              <Icon aria-hidden="true" className={cn('shrink-0', DENSITY[density].icon)} />
            )}
            <span aria-hidden="true" className="tabular-nums">
              {chip.count}
            </span>
          </span>
        )
      })}
    </>
  )
}
