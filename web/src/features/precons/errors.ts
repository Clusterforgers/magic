/** Plain words for every way a precon call can fail, and for a game that could not be made from a precon. */
import { describeApiError } from '@/features/create/apiErrors'
import { ApiError } from '@/net/api'
import { PreconError } from '@/net/precons'
import type { PreconFailureCode } from '@/net/precons'

// A `Record` over every code, so a code added to the client stops compiling here until it has words.
const MESSAGES: Record<PreconFailureCode, (error: PreconError) => string> = {
  not_found: () => 'The server has no precon with that code. Reload the list: it may have changed.',
  rate_limited: (error) =>
    error.retryAfter === null
      ? 'The server is answering too many requests from this address. Wait a moment, then try again.'
      : `The server is answering too many requests from this address. Try again in ${error.retryAfter} seconds.`,
  cards_unavailable: () => 'This server has no card data, so it has no precons to list.',
  unavailable: () => 'The server is shutting down or restarting. Wait a minute, then try again.',
  internal: () => 'The server ran into a problem. Try again in a moment.',
  unknown: (error) =>
    `The server refused the request, and this page does not know why. It said "${error.message}".`,
  network: () =>
    'Could not reach the server. Check your connection and that the game server is running, then try again.',
  bad_response: (error) =>
    `The server gave an answer this page could not read${error.status > 0 ? ` (status ${error.status})` : ''}. It may be down or restarting; try again in a moment.`,
}

/** What to tell a player about a failed precon call; anything that is not a `PreconError` gets a general apology. */
export function describePreconError(error: unknown): string {
  if (error instanceof PreconError) return MESSAGES[error.code](error)
  return 'Something unexpected went wrong. Try again.'
}

/** True when the server has no card data at all (503 `cards_unavailable`), which the page shows as a notice. */
export const isUnavailable = (error: unknown): boolean =>
  error instanceof PreconError && error.code === 'cards_unavailable'

/** Why a game could not be made from a precon, and whether asking again with the same name could work. */
export interface PlayFailure {
  text: string
  /** True when the precon itself is the trouble (the game does not take it, or it is gone): trying again will not help. */
  final: boolean
  /** True when the details list what stops the deck. */
  blocked: boolean
}

/** Words for a failed create with `deck: "precon:<code>"`. */
export function playFailure(error: unknown, name: string): PlayFailure {
  if (error instanceof ApiError) {
    if (error.status === 422) {
      return {
        text: `The game would not take ${name}. Open its details to see what stops it.`,
        final: true,
        blocked: true,
      }
    }
    if (error.code === 'unknown_deck') {
      return {
        text: `${name} is no longer on the server. Reload the list to see the precons it has now.`,
        final: true,
        blocked: false,
      }
    }
  }
  return { text: describeApiError(error), final: false, blocked: false }
}
