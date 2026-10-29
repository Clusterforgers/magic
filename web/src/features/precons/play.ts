/** The create-game request of a precon's Play button: the defaults of the create form, with the precon as seat 1's deck. */
import type { CreateGameRequest } from '@/net/api'
import type { PreconListItem } from '@/protocol/generated'

/** The deck id a seat names to play a precon (`precon:<code>`). */
export const deckOf = (precon: Pick<PreconListItem, 'code'>): string => `precon:${precon.code}`

/**
 * The body for `POST /api/games`. Two seats, listed in the game browser, and only the creator's seat held, with the
 * precon as its deck; the other seat is open and its player chooses a deck in the lobby. A deck with a commander needs
 * a Commander game, which the server judges the precon in; any other deck plays a Normal game.
 */
export function playRequest(
  precon: Pick<PreconListItem, 'code' | 'format'>,
  name: string,
): CreateGameRequest {
  return {
    format: precon.format === 'commander' ? 'commander' : 'normal',
    seats: [{ name: name.trim(), deck: deckOf(precon) }, { name: 'Player 2' }],
    visibility: 'public',
    held: 1,
  }
}
