/** The deck ids that ask the server to draw a deck at random, and their words. */

/** A precon drawn at random from those that suit the table (`RANDOM_PRECON_DECK` on the server). */
export const RANDOM_PRECON = 'random:precon'

/** One of the player's stored decks drawn at random (`RANDOM_OWN_DECK`); `:<group id>` narrows it to a group. */
export const RANDOM_MINE = 'random:mine'

/** What the lobby calls each draw until the game starts, as the server names it. */
export const RANDOM_PRECON_NAME = 'Random precon'
export const RANDOM_MINE_NAME = 'Random deck'

/** The id that draws one of the player's decks, from `group` when one is given. */
export const randomMineId = (group: string | null): string =>
  group === null ? RANDOM_MINE : `${RANDOM_MINE}:${group}`

/** Whether a deck id asks for a draw rather than naming a deck. */
export const isRandomId = (id: string | null | undefined): boolean => id?.startsWith('random:') === true
