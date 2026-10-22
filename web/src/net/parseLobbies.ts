/**
 * The answers of the game browser endpoints and of the game lookup, checked down to their leaves. Each reader returns
 * `null` for a body that does not have the documented shape, so a caller never holds a half-typed value; unknown extra
 * fields are dropped.
 */
import type { GameInfo, GameSeat, Joined, LobbyGame, SeatDeck, Visibility } from '@/protocol/generated'
import { isCount, isFlag, isListOf, isRecord, isText } from './check'
import { isGameResult } from './parse'

const VISIBILITIES: Record<Visibility, true> = { public: true, invite: true }

/** True for the two visibilities a game has. */
export const isVisibility = (value: unknown): value is Visibility =>
  isText(value) && Object.hasOwn(VISIBILITIES, value)

// `YYYY-MM-DDTHH:MM:SSZ`, the layout the server writes; the page puts it in a `datetime` attribute.
const STAMP = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/

/** One row of the listing, or `null` when a field is missing, mistyped or impossible (more seats filled than exist). */
export function readLobbyGame(item: unknown): LobbyGame | null {
  if (!isRecord(item)) return null
  const { game, host, format, filled, total, created_at: createdAt, age_secs: age } = item
  if (!isText(game) || !isText(host) || !isText(format)) return null
  if (!isCount(filled) || !isCount(total) || !isCount(age) || filled > total) return null
  if (!isText(createdAt) || !STAMP.test(createdAt)) return null
  return { game, host, format, filled, total, created_at: createdAt, age_secs: age }
}

/** The rows of `GET /api/lobbies`, or `null` when the body or any row is unreadable. */
export function readLobbyList(body: unknown): LobbyGame[] | null {
  if (!isRecord(body) || !isListOf(body.games, isRecord)) return null
  const games: LobbyGame[] = []
  for (const item of body.games) {
    const game = readLobbyGame(item)
    if (game === null) return null
    games.push(game)
  }
  return games
}

/**
 * The seat a join gave and the game it is in (the only place a join by code learns the id). `token` is `null` when the
 * account already held the seat, and must be there either way.
 */
export function readJoined(body: unknown): Joined | null {
  if (!isRecord(body) || !isText(body.game) || body.game === '' || !isCount(body.seat)) return null
  const { token } = body
  if (token !== null && !isText(token)) return null
  return { game: body.game, seat: body.seat, token }
}

/** True for the deck a seat shows the table: its name and format and how many cards need manual steps. */
export const isSeatDeck = (value: unknown): value is SeatDeck =>
  isRecord(value) && isText(value.name) && isText(value.format) && isCount(value.manual_cards)

/** What `PUT /api/games/{id}/seats/{n}/deck` answers: the three facts the table now knows of the seat's deck. */
export function readSeatDeck(body: unknown): SeatDeck | null {
  return isSeatDeck(body) ? { name: body.name, format: body.format, manual_cards: body.manual_cards } : null
}

/** The visibility `PATCH /api/games/{id}` answers with. */
export function readVisibility(body: unknown): Visibility | null {
  return isRecord(body) && isVisibility(body.visibility) ? body.visibility : null
}

function readGameSeat(item: unknown): GameSeat | null {
  return isRecord(item) && isCount(item.seat) && isText(item.name)
    ? { seat: item.seat, name: item.name }
    : null
}

/**
 * The answer of `GET /api/games/{id}`. A server from before the lobby showed readiness sends no `ready` (nobody is
 * known to be ready then) and one from before visibility sends no `visibility` (it listed nothing, so its games were
 * invite-only) and one from before Commander sends no `format` (it played Normal only); a field that is there must be right.
 */
export function readGameInfo(body: unknown): GameInfo | null {
  if (!isRecord(body) || !isFlag(body.started) || !Array.isArray(body.seats)) return null
  const format = body.format === undefined ? 'normal' : body.format
  if (!isText(format) || format === '') return null
  if (!isListOf(body.connected, isCount)) return null
  const ready = body.ready === undefined ? [] : body.ready
  if (!isListOf(ready, isCount)) return null
  const visibility = body.visibility === undefined ? 'invite' : body.visibility
  if (!isVisibility(visibility)) return null
  const result = body.result === null || isGameResult(body.result) ? body.result : undefined
  if (result === undefined) return null
  const seats: GameSeat[] = []
  for (const item of body.seats) {
    const seat = readGameSeat(item)
    if (seat === null) return null
    seats.push(seat)
  }
  return { format, started: body.started, seats, connected: body.connected, ready, result, visibility }
}
