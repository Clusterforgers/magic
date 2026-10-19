/** Plain-words messages for every way a card call can fail. */
import { CardsError } from '@/net/cards'
import { DeckError } from '@/net/decks'
import { describeDeckError } from '@/features/decks/errors'

/** What to tell a player about a failed card call; the deck words cover the codes both endpoints share. */
export function describeCardsError(error: unknown): string {
  if (!(error instanceof CardsError)) return describeDeckError(error)
  if (error.code === 'not_found') return 'That card is not in the database.'
  if (error.code === 'cards_unavailable') {
    return 'This server has no card database, so cards cannot be searched.'
  }
  return describeDeckError(new DeckError(error.code, error.status, error.message))
}
