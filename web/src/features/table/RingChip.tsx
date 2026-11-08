/** The chip of a seat's pod that says its Ring level and Ring-bearer. */
import { Circle } from 'lucide-react'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { PlayerView } from '@/protocol/generated'
import { DENSITY } from './PodParts'
import type { ChipDensity } from './PodParts'
import { ringWords } from './ring'

/** "Ring 2" with the bearer's name in the tooltip and the spoken name; nothing while the Ring has not tempted the seat. */
export function RingChip({
  owner,
  player,
  density = 'line',
}: {
  owner: string
  player: PlayerView
  density?: ChipDensity
}) {
  const bearer = usePlay((state) =>
    player.ring_bearer === undefined ? undefined : state.objectsById[player.ring_bearer]?.name,
  )
  if (player.ring_level === undefined) return null
  const words = ringWords(owner, player.ring_level, bearer)
  return (
    <span
      role="img"
      aria-label={words}
      title={words}
      data-slot="ring"
      className={cn(DENSITY[density].chip, 'text-muted-foreground')}
    >
      <Circle aria-hidden="true" className={cn('shrink-0', DENSITY[density].icon)} />
      <span aria-hidden="true" className="tabular-nums">
        {player.ring_level}
      </span>
    </span>
  )
}
