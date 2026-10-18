import { useCallback } from 'react'
import { useGame } from '@/features/game/gameContext'
import { seatName } from '@/features/game/useGameInfo'
import type { PlayerId } from '@/protocol/generated'

/** Turns a seat number into the player's name ("Seat 3" until the server has told us). */
export type SeatNamer = (seat: PlayerId) => string

/** The seat-name function of the game around the caller; it changes only when the lookup does. */
export function useSeatNamer(): SeatNamer {
  const { lookup } = useGame()
  return useCallback((seat) => seatName(lookup, seat), [lookup])
}
