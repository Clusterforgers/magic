/** Plain-words messages for every way a deck call can fail. */
import { DeckError } from '@/net/decks'
import type { DeckFailureCode } from '@/net/decks'

const OUT_OF_DATE =
  'This page and the server do not agree on how to talk to each other. Reload the page; if that does not help, the app needs an update.'

// The server's own sentence says which field or card is wrong; it is shown whole, as a sentence.
const sentence = (text: string): string => {
  const trimmed = text.trim()
  if (trimmed === '') return ''
  const capital = trimmed.charAt(0).toUpperCase() + trimmed.slice(1)
  return /[.!?]$/.test(capital) ? capital : `${capital}.`
}

// A `Record` over every code, so a code added in Rust (and so to the generated union) stops this file compiling.
const MESSAGES: Record<DeckFailureCode, (error: DeckError) => string> = {
  invalid_request: (error) => `The server could not use that request. ${sentence(error.message)}`,
  invalid_deck: (error) => `The server did not accept the deck. ${sentence(error.message)}`,
  unknown_cards: (error) =>
    `Some cards are not in the server's card database, so the deck was not saved. ${sentence(error.message)}`,
  not_found: () => 'That deck is no longer on the server. Reload the list to see the current decks.',
  deck_limit: () => 'You keep as many decks as an account may. Delete one to make room, then try again.',
  export_failed: (error) => `The deck could not be exported in that format. ${sentence(error.message)}`,
  unauthenticated: () => 'Your session has ended. Sign in again to see and change your decks.',
  forbidden_origin: () =>
    "The server does not accept requests from this web address. Open the page from the address the server's owner gave you.",
  payload_too_large: () => 'That is bigger than the server accepts. Use a shorter list.',
  unsupported_media_type: () => OUT_OF_DATE,
  decks_unavailable: () => 'This server keeps no decks, because it runs without a database.',
  cards_unavailable: () => 'This server has no card database, so it cannot read or check decks.',
  unavailable: () => 'The server is shutting down or restarting. Wait a minute, then try again.',
  internal: () => 'The server ran into a problem. Try again in a moment.',
  rate_limited: () => 'Too many requests came from this address. Wait a moment, then try again.',
  unknown: (error) =>
    `The server refused the request, and this page does not know why. It said "${error.message}".`,
  network: () =>
    'Could not reach the server. Check your connection and that the game server is running, then try again.',
  bad_response: (error) =>
    `The server gave an answer this page could not read${error.status > 0 ? ` (status ${error.status})` : ''}. It may be down or restarting; try again in a moment.`,
}

/** What to tell a player about a failed call; anything that is not a `DeckError` gets a general apology. */
export function describeDeckError(error: unknown): string {
  if (error instanceof DeckError) return MESSAGES[error.code](error)
  return 'Something unexpected went wrong. Try again.'
}

/** True when the server keeps no decks at all (503 `decks_unavailable`), which a page shows as a notice. */
export const isDecksUnavailable = (error: unknown): boolean =>
  error instanceof DeckError && error.code === 'decks_unavailable'

/** True when the session behind the cookie has ended (401 `unauthenticated`). */
export const isSessionEnded = (error: unknown): boolean =>
  error instanceof DeckError && error.code === 'unauthenticated'
