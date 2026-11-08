/** The chip of a seat's pod that says its speed (CR 702.179). */
import { Gauge } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { PlayerView } from '@/protocol/generated'
import { DENSITY } from './PodParts'
import type { ChipDensity } from './PodParts'
import { MAX_SPEED, speedWords } from './speed'

/** The speed with a gauge, highlighted at max speed; nothing while the seat has no speed. */
export function SpeedChip({
  owner,
  player,
  density = 'line',
}: {
  owner: string
  player: PlayerView
  density?: ChipDensity
}) {
  if (player.speed === undefined || player.speed === 0) return null
  const words = speedWords(owner, player.speed)
  const max = player.speed >= MAX_SPEED
  return (
    <span
      role="img"
      aria-label={words}
      title={words}
      data-slot="speed"
      data-max={max ? 'true' : undefined}
      className={cn(DENSITY[density].chip, max ? 'text-primary font-semibold' : 'text-muted-foreground')}
    >
      <Gauge aria-hidden="true" className={cn('shrink-0', DENSITY[density].icon)} />
      <span aria-hidden="true" className="tabular-nums">
        {player.speed}
      </span>
    </span>
  )
}
