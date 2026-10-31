/** Presence in words: which seats are away and how long before the game acts for them. */
import type { PlayState } from '@/play/store'
import type { PlayerId } from '@/protocol/generated'
import { secondsLeft } from './prompt/clockText'
import type { Waiting } from './turnText'

/** A seat that is away, with the milliseconds before the game acts for it (null when it has no clock). */
export interface AwaySeat {
  seat: PlayerId
  leftMs: number | null
}

/** The mark on the pod of a seat whose client is gone. */
export const AWAY_MARK = 'Away'

/** Whether `seat` is away: still in the game, with no client connected. A seat that left the game is not "away". */
export function isAway(state: PlayState, seat: PlayerId): boolean {
  const player = state.view?.players.find((p) => p.seat === seat)
  return player !== undefined && player.lost === null && !player.connected
}

/**
 * The seats the line under the banner mentions as away: those the game waits for, then any other seat that is away on
 * a clock (a defender who must block, say), in seat order within each group.
 */
export function awaySeats(state: PlayState, waiting: Waiting): AwaySeat[] {
  const waited = waiting.kind === 'others' ? waiting.seats : []
  const clocked = Object.entries(state.away)
    .filter(([, leftMs]) => leftMs !== null)
    .map(([seat]) => Number(seat))
    .filter((seat) => !waited.includes(seat))
    .sort((a, b) => a - b)
  return [...waited, ...clocked]
    .filter((seat) => isAway(state, seat))
    .map((seat) => ({ seat, leftMs: state.away[seat] ?? null }))
}

/** "Bob is away: the game acts for them in 12 s"; several seats are joined with "; ". Empty when nobody is away. */
export function awayText(seats: readonly AwaySeat[], name: (seat: PlayerId) => string): string {
  return seats
    .map(({ seat, leftMs }) =>
      leftMs === null
        ? `${name(seat)} is away`
        : `${name(seat)} is away: the game acts for them in ${secondsLeft(leftMs)} s`,
    )
    .join('; ')
}

/** The same seats without their times, for words that must not change every second. */
export function untimed(seats: readonly AwaySeat[]): AwaySeat[] {
  return seats.map(({ seat }) => ({ seat, leftMs: null }))
}
