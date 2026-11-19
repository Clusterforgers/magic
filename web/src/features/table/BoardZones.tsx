import { useSeatAnchor } from '@/features/learn/seatAnchors'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import type { PlayerId } from '@/protocol/generated'
import { commandCardsOf, exileOf, NO_IDS, playerOf, shownHandOf } from './selectors'
import { ZoneButton } from './ZoneButton'

/**
 * The zone chips of a board drawn without a pod (a hero beside yours, a phone's board under a tile): the hand the viewer
 * sees of another seat (a teammate's), then the graveyard, exile, command zone and Satchel that hold a card, each
 * opening its list as a pod's chip does. The tile above the board has the counts; it is one button and holds none.
 */
export function BoardZones({ seat, owner }: { seat: PlayerId; owner: string }) {
  const player = usePlay((state) => playerOf(state, seat))
  const you = usePlay((state) => state.seat === seat)
  const graveyardCoach = useSeatAnchor(seat, 'graveyard')
  const exile = usePlay(useShallow((state) => exileOf(state, seat)))
  const commanders = usePlay(useShallow((state) => commandCardsOf(state, seat)))
  if (player === undefined) return null
  const hand = shownHandOf(player, you)
  const graveyard = player.graveyard.cards ?? NO_IDS
  const satchel = player.satchel ?? NO_IDS
  const listed = !you && commanders.length > 0
  // Nothing to open draws nothing, so the board's top line stays empty and hidden.
  if (hand === null && graveyard.length + exile.length + satchel.length === 0 && !listed) return null
  return (
    <div
      role="group"
      aria-label={`${owner}'s zones`}
      data-slot="board-zones"
      className="flex items-center gap-1"
    >
      {hand === null ? null : <ZoneButton zone="hand" owner={owner} ids={hand} density="line" />}
      {graveyard.length > 0 ? (
        <ZoneButton zone="graveyard" owner={owner} ids={graveyard} density="line" coach={graveyardCoach} />
      ) : null}
      {exile.length > 0 ? <ZoneButton zone="exile" owner={owner} ids={exile} density="line" /> : null}
      {/* Your own commanders are cast from the hand dock; a pod-less board of another seat lists them here. */}
      {listed ? <ZoneButton zone="command" owner={owner} ids={commanders} density="line" /> : null}
      {satchel.length > 0 ? (
        <ZoneButton zone="satchel" owner={owner} ids={satchel} marks={player.boss?.satchel} density="line" />
      ) : null}
    </div>
  )
}
