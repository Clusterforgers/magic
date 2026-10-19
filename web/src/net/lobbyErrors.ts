/**
 * Failures of the game browser calls (`docs/server.md`, Game browser endpoints): the server's `LobbyErrorCode`, plus
 * `unknown`, `network` and `bad_response` raised locally, and a sentence for each that a player can read.
 */
import type { LobbyErrorCode } from '@/protocol/generated'
import { isRecord, isText } from './check'

// A `Record` over every code, so a code added in Rust (and so to the generated union) stops this file compiling.
const LOBBY_CODES: Record<LobbyErrorCode, true> = {
  invalid_request: true,
  invalid_name: true,
  unauthenticated: true,
  forbidden_origin: true,
  forbidden: true,
  not_found: true,
  full: true,
  started: true,
  not_claimed: true,
  payload_too_large: true,
  unsupported_media_type: true,
  too_many_attempts: true,
  rate_limited: true,
  accounts_unavailable: true,
  lobbies_unavailable: true,
  unavailable: true,
  internal: true,
}

/** True for a code the game browser endpoints send. */
export const isLobbyErrorCode = (value: string): value is LobbyErrorCode => Object.hasOwn(LOBBY_CODES, value)

/** Every `error` code the game browser endpoints send (`LobbyErrorCode`). */
export const LOBBY_ERROR_CODES: readonly LobbyErrorCode[] = Object.keys(LOBBY_CODES).filter(isLobbyErrorCode)

/** The codes of a `LobbyError`: the server's, plus `unknown`, `network` and `bad_response` raised locally. */
export type LobbyFailureCode = LobbyErrorCode | 'unknown' | 'network' | 'bad_response'

/** A failed game browser call. `message` is the server's own sentence or ours; show `describeLobbyError` instead. */
export class LobbyError extends Error {
  /** What went wrong, for code to act on. */
  readonly code: LobbyFailureCode
  /** The HTTP status, or 0 when no answer arrived (or the request was never sent). */
  readonly status: number
  /** Seconds the server asked the client to wait (`Retry-After` of a 429), or null. */
  readonly retryAfter: number | null

  constructor(code: LobbyFailureCode, status: number, message: string, retryAfter: number | null = null) {
    super(message)
    this.name = 'LobbyError'
    this.code = code
    this.status = status
    this.retryAfter = retryAfter
  }
}

// `Retry-After` is whole seconds or an HTTP date; the server sends seconds, and a date is simply not used.
function waitOf(response: Response): number | null {
  const value = response.headers.get('retry-after')
  return value !== null && /^\d{1,9}$/.test(value.trim()) ? Number(value.trim()) : null
}

/** The error for a non-success answer: the server's code and message if it sent `{error, message}`, else `bad_response`. */
export function lobbyFailure(response: Response, body: unknown): LobbyError {
  if (isRecord(body) && isText(body.error) && isText(body.message)) {
    const code = isLobbyErrorCode(body.error) ? body.error : 'unknown'
    return new LobbyError(code, response.status, body.message, waitOf(response))
  }
  return new LobbyError(
    'bad_response',
    response.status,
    `The server answered with HTTP ${response.status} and no readable error.`,
  )
}

const OUT_OF_DATE =
  'This page and the server do not agree on how to talk to each other. Reload the page; if that does not help, the app needs an update.'

/** "in 12 seconds", "in 1 second", or "in a minute" when the server did not say. */
function inWords(seconds: number | null): string {
  if (seconds === null) return 'in a minute'
  return seconds === 1 ? 'in 1 second' : `in ${seconds} seconds`
}

const MESSAGES: Record<LobbyFailureCode, (error: LobbyError) => string> = {
  invalid_request: () => 'The server could not use that request. Reload the page and try again.',
  invalid_name: () => 'Choose a name of 1 to 32 characters, without control or invisible characters.',
  unauthenticated: () =>
    'You are not signed in, and this page has no seat link for the game. Sign in, or open the game from your own seat link.',
  forbidden_origin: () =>
    "The server does not accept requests from this web address. Open the page from the address the server's owner gave you.",
  forbidden: () => 'Only the host of the game can do that.',
  not_found: () =>
    'No open game matches that. It may have started or closed, the code may be wrong, or you may no longer have a seat in it.',
  full: () => 'Someone took that seat first: the game is full.',
  started: () => 'That game has already started.',
  not_claimed: () =>
    'That seat was made with the game, not taken through the game browser or a code, so it cannot be given back.',
  payload_too_large: () => 'The request was too big for the server. Use a shorter name.',
  unsupported_media_type: () => OUT_OF_DATE,
  too_many_attempts: (error) =>
    `Too many wrong codes from this address. Try again ${inWords(error.retryAfter)}.`,
  rate_limited: (error) =>
    `The list was asked for too often. It can be asked for again ${inWords(error.retryAfter)}.`,
  accounts_unavailable: () =>
    'This server runs without accounts, so it cannot tell who you are. Use your seat link instead.',
  lobbies_unavailable: () => 'The server cannot list its games right now. Try again in a moment.',
  unavailable: () => 'The server is busy or restarting. Wait a minute, then try again.',
  internal: () => 'The server ran into a problem. Try again in a moment.',
  unknown: (error) =>
    `The server refused the request, and this page does not know why. It said "${error.message}".`,
  network: () =>
    'Could not reach the server. Check your connection and that the game server is running, then try again.',
  bad_response: (error) =>
    `The server gave an answer this page could not read${error.status > 0 ? ` (status ${error.status})` : ''}. It may be down or restarting; try again in a moment.`,
}

/** What to tell a player about a failed call; anything that is not a `LobbyError` gets a general apology. */
export function describeLobbyError(error: unknown): string {
  if (error instanceof LobbyError) return MESSAGES[error.code](error)
  return 'Something unexpected went wrong. Try again.'
}
