/** Words for the "My games" list: who else is in the game, how a finished one ended, and what the page counts. */
import type { MyGame, MyGameSeat } from '@/protocol/generated'

/** The name to show for another seat: the username of the account sitting there, else the name the seat was given. */
export const playerLabel = (seat: MyGameSeat): string => seat.user ?? seat.name

/** "Bo", "Bo and Cy", "Bo, Cy and Di". */
export function joinNames(names: readonly string[]): string {
  if (names.length <= 1) return names.join('')
  return `${names.slice(0, -1).join(', ')} and ${names.at(-1) ?? ''}`
}

/** The other players of a game, by username for accounts and by seat name for guests. */
export const othersText = (game: MyGame): string => joinNames(game.others.map(playerLabel))

/**
 * How a finished game ended from the user's seat: "You won", "Bo won", "Draw"; a finished game the server holds no
 * result for says so in words; null while the game is not over.
 */
export function resultText(game: MyGame): string | null {
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
}

/** "No games", "1 game", "3 games": the count the page announces. */
export const gamesText = (count: number): string =>
  count === 0 ? 'No games' : `${count} ${count === 1 ? 'game' : 'games'}`

/** What a Continue link is called for a screen reader: the visible word, then which game. */
export function continueLabel(game: MyGame): string {
  const others = othersText(game)
  return others === '' ? 'Continue your game' : `Continue your game with ${others}`
}
