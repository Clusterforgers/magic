/** A seat of the result page shown by its commander's art crop in the seat's colour, as at the table. */
import { useShallow } from 'zustand/react/shallow'
import { CommanderPortrait, portraitIdentities } from '@/features/table/CommanderPortrait'
import { SeatDot, seatHueStyle } from '@/features/table/PodParts'
import { commandCardsOf } from '@/features/table/selectors'
import type { PlayerId } from '@/protocol/generated'
import { usePlay } from './gameContext'

/** The seat's commander portrait, or its dot where the commander has no picture. Decoration: the row names the seat. */
export function SeatFace({ seat }: { seat: PlayerId }) {
  const commanders = usePlay(useShallow((state) => commandCardsOf(state, seat)))
  const pictured = usePlay((state) => portraitIdentities(commanders, state.objectsById).length > 0)
  return (
    <span
      aria-hidden="true"
      data-slot="seat-face"
      style={seatHueStyle(seat)}
      className="flex size-8 shrink-0 items-center justify-center"
    >
      {!pictured ? <SeatDot seat={seat} /> : <CommanderPortrait ids={commanders} className="size-8" />}
    </span>
  )
}
