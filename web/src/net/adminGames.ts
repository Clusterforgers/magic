/**
 * The admin's game calls: the live lobbies and running games (`GET /api/admin/games`) and ending one
 * (`POST /api/admin/games/{id}/end`). Failures reject with an `AccountError`, as the other admin calls do.
 */
import type { AdminGame, AdminGameList, AdminGameSeat, EndGameBody } from '@/protocol/generated'
import { AccountError, isAuthErrorCode } from './accounts'
import { isFlag, isListOf, isRecord, isText } from './check'

type Fetch = (input: string, init?: RequestInit) => Promise<Response>

/** The admin's game calls; each rejects with an `AccountError`. */
export interface AdminGamesApi {
  /** Every lobby and running game, newest first; rejects with `forbidden` unless the caller is an admin. */
  listGames(): Promise<AdminGame[]>
  /** Ends a game for good; a game that already ended is fine. */
  endGame(game: string, reason: string): Promise<void>
}

const isSeat = (value: unknown): value is AdminGameSeat =>
  isRecord(value) && isText(value.name) && isFlag(value.connected)

function isGame(value: unknown): value is AdminGame {
  return (
    isRecord(value) &&
    isText(value.game) &&
    isText(value.format) &&
    isText(value.visibility) &&
    isFlag(value.started) &&
    isFlag(value.tutorial) &&
    isListOf(value.seats, isSeat) &&
    isText(value.created_at) &&
    isText(value.last_activity)
  )
}

function readList(body: unknown): AdminGame[] | null {
  if (!isRecord(body) || !isListOf(body.games, isGame)) return null
  return (body as AdminGameList).games
}

async function failure(response: Response): Promise<AccountError> {
  let body: unknown
  try {
    body = JSON.parse(await response.text())
  } catch {
    body = undefined
  }
  if (isRecord(body) && isText(body.error) && isText(body.message)) {
    return new AccountError(
      isAuthErrorCode(body.error) ? body.error : 'unknown',
      response.status,
      body.message,
    )
  }
  return new AccountError(
    'bad_response',
    response.status,
    `The server answered with HTTP ${response.status}.`,
  )
}

/** Builds the client; `fetch` defaults to the global one, looked up at call time. */
export function createAdminGamesApi(options: { fetch?: Fetch; baseUrl?: string } = {}): AdminGamesApi {
  const base = options.baseUrl ?? ''

  async function call(path: string, init: RequestInit): Promise<Response> {
    const send: Fetch = options.fetch ?? ((url, request) => globalThis.fetch(url, request))
    let response: Response
    try {
      response = await send(`${base}${path}`, { credentials: 'include', ...init })
    } catch {
      throw new AccountError('network', 0, 'Could not reach the server. Check your connection and try again.')
    }
    if (!response.ok) throw await failure(response)
    return response
  }

  return {
    async listGames() {
      const response = await call('/api/admin/games', {})
      let body: unknown
      try {
        body = JSON.parse(await response.text())
      } catch {
        body = undefined
      }
      const games = readList(body)
      if (games === null) {
        throw new AccountError(
          'bad_response',
          response.status,
          'The server sent an answer this page cannot read.',
        )
      }
      return games
    },
    async endGame(game, reason) {
      const trimmed = reason.trim()
      const body: EndGameBody = trimmed === '' ? {} : { reason: trimmed }
      await call(`/api/admin/games/${encodeURIComponent(game)}/end`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(body),
      })
    },
  }
}

/** The client for the page origin. */
export const adminGamesApi: AdminGamesApi = createAdminGamesApi()
