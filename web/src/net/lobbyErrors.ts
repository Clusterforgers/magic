/**
 * Failures of the game browser calls (`docs/server.md`, Game browser endpoints): the server's `LobbyErrorCode`, the
 * `DeckChoiceErrorCode` of a request that names a deck, plus `unknown`, `network` and `bad_response` raised locally, and
 * a sentence for each that a player can read.
 */
import type { DeckChoiceErrorCode, Issue, LobbyErrorCode } from '@/protocol/generated'
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

// The codes of a request that names a deck, a type of their own on the server so that older clients keep working.
const DECK_CHOICE_CODES: Record<DeckChoiceErrorCode, true> = {
  unknown_deck: true,
  deck_blocked: true,
  seat_ready: true,
}

/** True for a code the game browser endpoints send. */
export const isLobbyErrorCode = (value: string): value is LobbyErrorCode => Object.hasOwn(LOBBY_CODES, value)

/** Every `error` code the game browser endpoints send (`LobbyErrorCode`). */
export const LOBBY_ERROR_CODES: readonly LobbyErrorCode[] = Object.keys(LOBBY_CODES).filter(isLobbyErrorCode)

/** True for a code that only a request naming a deck sends: a join, the creation of a game, `PUT .../deck`. */
export const isDeckChoiceErrorCode = (value: string): value is DeckChoiceErrorCode =>
  Object.hasOwn(DECK_CHOICE_CODES, value)

/** Every `error` code that only a request naming a deck sends (`DeckChoiceErrorCode`). */
export const DECK_CHOICE_ERROR_CODES: readonly DeckChoiceErrorCode[] =
  Object.keys(DECK_CHOICE_CODES).filter(isDeckChoiceErrorCode)

/** The codes of a `LobbyError`: the server's, plus `unknown`, `network` and `bad_response` raised locally. */
export type LobbyFailureCode = LobbyErrorCode | DeckChoiceErrorCode | 'unknown' | 'network' | 'bad_response'

/** A failed game browser call. `message` is the server's own sentence or ours; show `describeLobbyError` instead. */
export class LobbyError extends Error {
  /** What went wrong, for code to act on. */
  readonly code: LobbyFailureCode
  /** The HTTP status, or 0 when no answer arrived (or the request was never sent). */
  readonly status: number
  /** Seconds the server asked the client to wait (`Retry-After` of a 429), or null. */
  readonly retryAfter: number | null
  /** What breaks the rules, for a refused deck (422 `deck_blocked`); empty for every other failure. */
  readonly issues: readonly Issue[]

  constructor(
    code: LobbyFailureCode,
    status: number,
    message: string,
    retryAfter: number | null = null,
    issues: readonly Issue[] = [],
  ) {
    super(message)
    this.name = 'LobbyError'
    this.code = code
    this.status = status
    this.retryAfter = retryAfter
    this.issues = issues
  }
}

// `Retry-After` is whole seconds or an HTTP date; the server sends seconds, and a date is simply not used.
function waitOf(response: Response): number | null {
  const value = response.headers.get('retry-after')
  return value !== null && /^\d{1,9}$/.test(value.trim()) ? Number(value.trim()) : null
}

const isNullableText = (value: unknown): value is string | null => value === null || isText(value)

// The issues of a refused deck (`DeckBlockedBody`); an item that is not an issue is left out, never half-read.
function issuesOf(body: Record<string, unknown>): Issue[] {
  if (!Array.isArray(body.issues)) return []
  return body.issues.filter(
    (item): item is Issue =>
      isRecord(item) &&
      isText(item.code) &&
      isText(item.message) &&
      isNullableText(item.zone) &&
      isNullableText(item.oracle_id) &&
      isNullableText(item.name),
  )
}

/**
 * The error for a non-success answer: the server's code and message if it sent `{error, message}`, else
 * `bad_response`. A refused deck (`deck_blocked`) keeps its issue list.
 */
export function lobbyFailure(response: Response, body: unknown): LobbyError {
  if (isRecord(body) && isText(body.error) && isText(body.message)) {
    const code = isLobbyErrorCode(body.error) || isDeckChoiceErrorCode(body.error) ? body.error : 'unknown'
    const issues = code === 'deck_blocked' ? issuesOf(body) : []
    return new LobbyError(code, response.status, body.message, waitOf(response), issues)
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
  unknown_deck: () =>
    'That deck is not available: it was deleted, or it is not in the account you are signed in with. Choose another.',
  deck_blocked: () => 'That deck cannot be played in this game.',
  seat_ready: () => 'This seat is ready, so its deck cannot change. Take Ready back first.',
  unknown: (error) =>
    `The server refused the request, and this page does not know why. It said "${error.message}".`,
  network: () =>
    'Could not reach the server. Check your connection and that the game server is running, then try again.',
  bad_response: (error) =>
    `The server gave an answer this page could not read${error.status > 0 ? ` (status ${error.status})` : ''}. It may be down or restarting; try again in a moment.`,
}

/** What a lobby control was doing when it failed; a few codes read better with that said. */
export type LobbyAbout = 'visibility' | 'close' | 'remove' | 'leave' | 'deck'

const GONE = 'This game is no longer open. The host may have closed it, or it may have started.'

// Only the codes whose general sentence talks about joining or a code have their own here; the rest read the same.
const ABOUT: Record<LobbyAbout, Partial<Record<LobbyFailureCode, string>>> = {
  visibility: {
    forbidden: 'Only the host can change who can find the game.',
    not_found: GONE,
    started: 'The game has already started, so who can find it can no longer change.',
  },
  close: {
    forbidden: 'Only the host can close the game.',
    not_found: 'This game is already closed or gone.',
    started: 'The game has already started, so it can no longer be closed from the lobby.',
  },
  remove: {
    forbidden: 'Only the host can remove a player.',
    not_found: GONE,
    started: 'The game has already started, so players can no longer be removed.',
    not_claimed:
      'That seat was made with the game, not taken through the game browser or a code, so its player cannot be removed.',
  },
  leave: {
    forbidden: 'You cannot give back a seat that is not yours.',
    not_found:
      'You no longer have a seat in this game. It may have been closed, or the host may have removed you.',
    started: 'The game has already started, so a seat can no longer be given back.',
    not_claimed:
      "This seat came with the game's own link, so it cannot be given back. Close this tab to stop playing it.",
  },
  deck: {
    forbidden: 'You can only choose the deck of your own seat.',
    not_found:
      'You no longer have a seat in this game. It may have been closed, or the host may have removed you.',
    started: 'The game has already started, so the deck can no longer change.',
    unknown_deck:
      'That deck is no longer there, or it is not in the account you are signed in with. Reload the page to see the current decks, then choose again.',
    seat_ready: 'You are ready, so your deck cannot change. Press Not ready, then choose again.',
  },
}

/**
 * What to tell a player about a failed call; anything that is not a `LobbyError` gets a general apology. `about` names
 * the lobby control that failed, for the codes whose general sentence talks about joining.
 */
export function describeLobbyError(error: unknown, about?: LobbyAbout): string {
  if (error instanceof LobbyError) {
    return (about === undefined ? undefined : ABOUT[about][error.code]) ?? MESSAGES[error.code](error)
  }
  return 'Something unexpected went wrong. Try again.'
}
