/** Words, counts, filters and codes for the open-games list; pure, so the page and its tests share them. */
import type { LobbyGame } from '@/protocol/generated'

const MINUTE = 60
const HOUR = 60 * MINUTE
const DAY = 24 * HOUR

/** "just now", "3 min ago", "2 h ago", "1 d ago": whole units, rounded down. */
export function ageText(seconds: number): string {
  if (seconds < MINUTE) return 'just now'
  if (seconds < HOUR) return `${Math.floor(seconds / MINUTE)} min ago`
  if (seconds < DAY) return `${Math.floor(seconds / HOUR)} h ago`
  return `${Math.floor(seconds / DAY)} d ago`
}

/** How old a row is at `now` (ms): the age the server gave when the rows arrived at `fetchedAt` plus the time since. */
export function ageNow(game: LobbyGame, fetchedAt: number, now: number): number {
  return game.age_secs + Math.max(0, Math.floor((now - fetchedAt) / 1000))
}

/** The format key as a player reads it: "normal" is "Normal", "free-for-all" is "Free for all". */
export function formatName(key: string): string {
  const words = key.replace(/[-_]+/g, ' ').trim()
  return words === '' ? key : `${words.charAt(0).toUpperCase()}${words.slice(1)}`
}

/** "2 of 4". */
export const seatsText = (game: LobbyGame): string => `${game.filled} of ${game.total}`

/** Whether every seat is taken. */
export const isFull = (game: LobbyGame): boolean => game.filled >= game.total

/** The name of a row's Join button: "Join Bo's game, 2 of 4 seats". */
export const joinLabel = (game: LobbyGame): string =>
  `Join ${game.host}'s game, ${game.filled} of ${game.total} seats`

/** The polite count line: "3 open games", "1 open game", and "2 of 12 open games" while a filter hides some. */
export function openGamesText(shown: number, all: number): string {
  const noun = all === 1 ? 'open game' : 'open games'
  return shown === all ? `${all} ${noun}` : `${shown} of ${all} ${noun}`
}

/** The filters above the list: an exact seat count, one format, and whether full games are hidden. */
export interface Filters {
  seats: number | null
  format: string | null
  hideFull: boolean
}

/** No filter set. */
export const NO_FILTERS: Filters = { seats: null, format: null, hideFull: false }

/** The filters appear when the listing holds more rows than this. */
export const FILTERS_ABOVE_ROWS = 8

/** Whether any filter is set. */
export const filtersActive = (filters: Filters): boolean =>
  filters.seats !== null || filters.format !== null || filters.hideFull

/** The rows a filter lets through, in their order. */
export function filterGames(games: readonly LobbyGame[], filters: Filters): LobbyGame[] {
  return games.filter(
    (game) =>
      (filters.seats === null || game.total === filters.seats) &&
      (filters.format === null || game.format === filters.format) &&
      !(filters.hideFull && isFull(game)),
  )
}

/** The seat counts present in the rows, smallest first. */
export const seatOptions = (games: readonly LobbyGame[]): number[] =>
  [...new Set(games.map((game) => game.total))].sort((a, b) => a - b)

/** The formats present in the rows, in alphabetical order. */
export const formatOptions = (games: readonly LobbyGame[]): string[] =>
  [...new Set(games.map((game) => game.format))].sort()

// The server's alphabet: capitals and digits without I, O, 0 and 1 (docs/server.md).
const CODE_SYMBOLS = /^[ABCDEFGHJKLMNPQRSTUVWXYZ23456789]{8}$/

/** A typed join code as the server reads it: capitals, with spaces and hyphens taken out. */
export const normalizeCode = (typed: string): string => typed.toUpperCase().replace(/[\s-]/g, '')

/** Whether text (as `normalizeCode` leaves it) can be a join code: 8 symbols of the server's alphabet. */
export const isJoinCode = (code: string): boolean => CODE_SYMBOLS.test(code)
