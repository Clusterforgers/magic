import { useGameStore } from '@/game/store'
import { zoneKey } from '@/game/zones'
import type { ObjectId, PlayerId, Zone } from '@/game/types'
import { Card } from './Card'

const EMPTY: readonly ObjectId[] = []

interface ActivateProps {
  onActivate: (id: ObjectId) => void
}

interface ZoneRowProps extends ActivateProps {
  player: PlayerId
  zone: Zone
  label: string
}

/** A single zone of one player as a wrapping row of cards. */
export function ZoneRow({ player, zone, label, onActivate }: ZoneRowProps) {
  const ids = useGameStore((state) => state.zones[zoneKey(player, zone)]) ?? EMPTY
  return (
    <section aria-label={`${label} of player ${player}`} className="flex flex-wrap gap-1">
      {ids.map((id) => (
        <Card key={id} id={id} onActivate={onActivate} />
      ))}
    </section>
  )
}

/** A player's seat: battlefield above hand. */
export function Seat({ player, onActivate }: ActivateProps & { player: PlayerId }) {
  return (
    <div className="flex flex-col gap-2 rounded-lg border p-2" data-testid={`seat-${player}`}>
      <h2 className="text-xs font-semibold text-muted-foreground">Player {player}</h2>
      <ZoneRow player={player} zone="battlefield" label="Battlefield" onActivate={onActivate} />
      <ZoneRow player={player} zone="hand" label="Hand" onActivate={onActivate} />
    </div>
  )
}
