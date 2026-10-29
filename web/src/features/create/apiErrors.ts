/** Plain-words messages for every way a REST call can fail. */
import { ApiError } from '@/net/api'
import type { ApiErrorCode } from '@/net/api'

const OUT_OF_DATE =
  'This page and the server do not agree on how to talk to each other. Reload the page; if that does not help, the app needs an update.'

// A `Record` over every code, so adding a code to the client stops compiling here until it has a message.
const MESSAGES: Record<ApiErrorCode, (error: ApiError) => string> = {
  invalid_request: () =>
    'The server could not use those game settings. Check that there are 2 to 12 seats (2 to 6 in Commander), that every seat has a name of 1 to 32 characters and that the house rules are in range.',
  unsupported_format: () =>
    'The server does not know that format. Reload the page, then choose Commander or Normal again.',
  unknown_deck: () =>
    'One of the chosen decks is no longer on the server. Reload the page to get the current decks, then pick again.',
  forbidden_origin: () =>
    "The server does not accept requests from this web address. Open the page from the address the server's owner gave you.",
  not_found: () =>
    'The server could not find what this page asked for. The page and the server may be out of date, so reload the page and try again.',
  method_not_allowed: () => OUT_OF_DATE,
  unsupported_media_type: () => OUT_OF_DATE,
  payload_too_large: () => 'The game settings are too big for the server. Use fewer seats or shorter names.',
  internal: () => 'The server ran into a problem. Try again in a moment.',
  server_full: () => 'The server is already running as many games as it allows. Try again in a few minutes.',
  unavailable: () => 'The server is shutting down or restarting. Wait a minute, then try again.',
  unknown: (error) =>
    `The server refused the request, and this page does not know why. It said "${error.message}".`,
  network: () =>
    'Could not reach the server. Check your connection and that the game server is running, then try again.',
  bad_response: (error) =>
    `The server gave an answer this page could not read${error.status > 0 ? ` (status ${error.status})` : ''}. It may be down or restarting; try again in a moment.`,
}

/** What to tell a player about a failed call; anything that is not an `ApiError` gets a general apology. */
export function describeApiError(error: unknown): string {
  if (error instanceof ApiError) return MESSAGES[error.code](error)
  return 'Something unexpected went wrong. Try again.'
}
