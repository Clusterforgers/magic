import { memo } from 'react'
import type { PlayerId } from '@/protocol/generated'
import { Battlefield } from './Battlefield'
import type { CardSize } from './cardSize'
import { SeatPanel } from './SeatPanel'

interface SeatBoardProps {
  seat: PlayerId
  size: CardSize
}

/** An opponent: the seat panel on top, then their battlefield with lands on the far edge. */
export const OpponentBoard = memo(function OpponentBoard({ seat, size }: SeatBoardProps) {
  return (
    <div className="grid min-w-0 content-start gap-2">
      <SeatPanel seat={seat} size={size} />
      <Battlefield seat={seat} size={size} facing="far" />
    </div>
  )
})

/** The viewer's own side: the battlefield with creatures toward the middle, then the panel with the hand. */
export const OwnBoard = memo(function OwnBoard({ seat, size }: SeatBoardProps) {
  return (
    <div className="grid min-w-0 content-start gap-2">
      <Battlefield seat={seat} size={size} facing="near" />
      <SeatPanel seat={seat} size={size} />
    </div>
  )
})
