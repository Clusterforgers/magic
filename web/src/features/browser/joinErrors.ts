/** What a failed join says to the player, and whether the seat is gone for good. */
import { describeLobbyError, LobbyError } from '@/net/lobbyErrors'

/** Why a join failed, in words, and whether asking again could help. */
export interface JoinFailure {
  text: string
  /** The seat cannot be had any more (taken, game started or gone): the dialog closes and the list is read again. */
  final: boolean
}

/**
 * The server answers one 404 for an unknown game, a game that is not public and a wrong code, so this one sentence
 * is all a row's 404 gets: it must not say which of them it was.
 */
export const NOT_OPEN = 'That game is not open any more. The list shows the games that are.'

/**
 * The one sentence for a join with a code that found no game. A wrong code, a malformed one and a game that is gone
 * share a 404 on the server, so it says no more than that nothing is open under the code.
 */
export const NO_GAME_WITH_CODE = 'No open game with that code.'

/** What a join code looks like, said to someone whose text cannot be one (checked before the server is asked). */
export const CODE_SHAPE = 'A code is 8 letters and digits, like K7M2-9QXA.'

/** The words for a failed `joinGame`; every other code keeps its sentence from `describeLobbyError`. */
export function joinFailure(error: unknown): JoinFailure {
  if (error instanceof LobbyError) {
    if (error.code === 'not_found') return { text: NOT_OPEN, final: true }
    if (error.code === 'full' || error.code === 'started') {
      return { text: describeLobbyError(error), final: true }
    }
  }
  return { text: describeLobbyError(error), final: false }
}

/** The words for a failed `joinByCode`: as for a row, but a 404 says that no open game has the code. */
export function codeFailure(error: unknown): JoinFailure {
  if (error instanceof LobbyError && error.code === 'not_found') {
    return { text: NO_GAME_WITH_CODE, final: true }
  }
  return joinFailure(error)
}
