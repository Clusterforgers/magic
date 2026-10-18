/** Words for the "My games" list: how long ago, who else is in the game, and how a finished one ended. */
import type { MyGame, MyGameSeat } from '@/protocol/generated'

const MINUTE = 60_000
const HOUR = 60 * MINUTE
const DAY = 24 * HOUR

// Spelled out rather than asked of `Intl`, whose short month names differ between versions ("Sep" and "Sept").
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']

const plural = (count: number, unit: string): string => `${count} ${unit}${count === 1 ? '' : 's'} ago`

/** "just now", "5 minutes ago", "3 hours ago", "2 days ago", and a date once it is a month old. */
export function timeSince(createdAt: string, now: number): string {
  const created = Date.parse(createdAt)
  if (Number.isNaN(created)) return 'some time ago'
  const age = now - created
  if (age < MINUTE) return 'just now'
  if (age < HOUR) return plural(Math.floor(age / MINUTE), 'minute')
  if (age < DAY) return plural(Math.floor(age / HOUR), 'hour')
  if (age < 30 * DAY) return plural(Math.floor(age / DAY), 'day')
  const date = new Date(created)
  return `on ${date.getUTCDate()} ${MONTHS[date.getUTCMonth()] ?? ''} ${date.getUTCFullYear()}`
}

/** The name to show for another seat: the username of the account sitting there, else the name the seat was given. */
export const playerLabel = (seat: MyGameSeat): string => seat.user ?? seat.name

/** "Bo", "Bo and Cy", "Bo, Cy and Di". */
export function joinNames(names: readonly string[]): string {
  if (names.length <= 1) return names.join('')
  return `${names.slice(0, -1).join(', ')} and ${names.at(-1) ?? ''}`
}

/** The other players of a game, by username for accounts and by seat name for guests. */
export const othersText = (game: MyGame): string => joinNames(game.others.map(playerLabel))

/** How a finished game ended from the user's seat: "You won", "Bo won", "Draw"; null while it is not over. */
export function resultText(game: MyGame): string | null {
  if (game.status !== 'finished') return null
  if (game.result === null) return 'Finished'
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
