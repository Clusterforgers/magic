/** Plain-words messages for a fix-mark call that failed. */
import { describeDeckError } from '@/features/decks/errors'
import { DeckError } from '@/net/decks'

/** Whether the failure means marks cannot be made here at all: no session, or a server without a card database. */
export function marksUnavailable(error: unknown): boolean {
  return (
    error instanceof DeckError &&
    (error.code === 'unauthenticated' || error.code === 'cards_unavailable' || error.code === 'decks_unavailable')
  )
}

/** What to tell a player about a failed mark; the deck messages cover the codes the endpoints share. */
export function describeMarkError(error: unknown): string {
  if (error instanceof DeckError) {
    if (error.code === 'not_found') return 'The server does not know this card, so it was not marked.'
    if (error.code === 'invalid_request') return 'The note was not accepted. Make it shorter or remove odd characters.'
    if (error.code === 'unauthenticated') return 'Your session has ended. Sign in again to mark cards.'
  }
  return describeDeckError(error)
}
