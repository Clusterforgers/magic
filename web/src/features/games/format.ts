/** Words for the "My games" list: who else is in the game, how a finished one ended, and what the page counts. */
import type { MyGame, MyGameSeat } from '@/protocol/generated'

/** The name to show for another seat: the username of the account sitting there, else the name the seat was given. */
export const playerLabel = (seat: MyGameSeat): string => seat.user ?? seat.name

/** "Bo", "Bo and Cy", "Bo, Cy and Di". */
export function joinNames(names: readonly string[]): string {
  if (names.length <= 1) return names.join('')
  return `${names.slice(0, -1).join(', ')} and ${names.at(-1) ?? ''}`
}

/**
 * The other players of a game, by username for accounts and by seat name for guests; seats nobody has taken yet have no
 * name and read as "an open seat" or "3 open seats", after the players.
 */
export function othersText(game: MyGame): string {
  const names = game.others.map(playerLabel).filter((name) => name !== '')
  const open = game.others.length - names.length
  if (open > 0) names.push(open === 1 ? 'an open seat' : `${open} open seats`)
  return joinNames(names)
}

/** What a failed game says when the answer carries no reason. */
export const NO_REASON = 'This game could not be restored, so it cannot continue.'

/**
 * How a finished game ended from the user's seat: "You won", "Bo won", "Draw"; a finished game the server holds no
 * result for says so in words; a failed game says why it could not be restored; null while the game is not over.
 */
export function resultText(game: MyGame): string | null {
  if (game.status === 'failed') return game.reason ?? NO_REASON
  if (game.status !== 'finished') return null
  if (game.result === null) return 'Ended without a result'
  if (game.result === 'Draw') return 'Draw'
  const winner = game.result.Winner
  if (winner === game.seat) return 'You won'
  const other = game.others.find((seat) => seat.seat === winner)
  return `${other === undefined ? `Seat ${winner + 1}` : playerLabel(other)} won`
}

/** The label of a game's state; the colour of its badge never carries the meaning alone. */
export const STATUS_LABEL: Record<MyGame['status'], string> = {
  lobby: 'Lobby',
  running: 'Running',
  finished: 'Finished',
  failed: 'Failed',
}

/** "No games", "1 game", "3 games": the count the page announces. */
export const gamesText = (count: number): string =>
  count === 0 ? 'No games' : `${count} ${count === 1 ? 'game' : 'games'}`

/** What a Continue link is called for a screen reader: the visible word, then which game. */
export function continueLabel(game: MyGame): string {
  const others = othersText(game)
  return others === '' ? 'Continue your game' : `Continue your game with ${others}`
}

/** What a Remove button is called for a screen reader: the visible word, then which game. */
export function removeLabel(game: MyGame): string {
  const others = othersText(game)
  return others === '' ? 'Remove your failed game' : `Remove your failed game with ${others}`
}
