import { useCallback } from 'react'
import { useGame, usePlay } from '@/features/game/gameContext'
import { seatName } from '@/features/game/useGameInfo'
import type { PlayerId } from '@/protocol/generated'
import { voiced } from './speech'

/** Turns a seat number into the player's name ("Seat 3" until the server has told us). */
export type SeatNamer = (seat: PlayerId) => string

/** The seat-name function of the game around the caller; it changes only when the lookup does. */
export function useSeatNamer(): SeatNamer {
  const { lookup } = useGame()
  return useCallback((seat) => seatName(lookup, seat), [lookup])
}

/** The seat the viewer plays, or null for a spectator and before the Welcome. */
export function useViewerSeat(): PlayerId | null {
  return usePlay((state) => (state.view?.spectator === true ? null : state.seat))
}

/** The seat namer that marks the viewer's own seat (`voiced`), for words `speak` turns to second person. */
export function useVoicedNamer(): SeatNamer {
  const name = useSeatNamer()
  const viewer = useViewerSeat()
  return useCallback((seat) => voiced(name, viewer)(seat), [name, viewer])
}
