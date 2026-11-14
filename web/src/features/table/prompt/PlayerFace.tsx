/** A player offered as a target or a choice, shown by their commander's art crop in their seat's colour beside the name. */
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import type { PlayerId } from '@/protocol/generated'
import { CommanderPortrait } from '../CommanderPortrait'
import { SeatDot, seatHueStyle } from '../PodParts'
import { commandCardsOf } from '../selectors'

/** The seat's commander portrait, or its dot where the commander has no picture. Decoration: the name says who. */
export function PlayerFace({ seat }: { seat: PlayerId }) {
  const commanders = usePlay(useShallow((state) => commandCardsOf(state, seat)))
  return (
    <span
      aria-hidden="true"
      data-slot="player-face"
      style={seatHueStyle(seat)}
      className="flex shrink-0 items-center"
    >
      {commanders.length === 0 ? (
        <SeatDot seat={seat} />
      ) : (
        <CommanderPortrait ids={commanders} className="size-6" />
      )}
    </span>
  )
}
