/**
 * The answers of the game browser endpoints, checked down to their leaves. Each reader returns `null` for a body that
 * does not have the documented shape, so a caller never holds a half-typed value; unknown extra fields are dropped.
 */
import type { Joined, LobbyGame, Visibility } from '@/protocol/generated'
import { isCount, isListOf, isRecord, isText } from './check'

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

/** The visibility `PATCH /api/games/{id}` answers with. */
export function readVisibility(body: unknown): Visibility | null {
  return isRecord(body) && isVisibility(body.visibility) ? body.visibility : null
}
