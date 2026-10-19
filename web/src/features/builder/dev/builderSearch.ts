import { DEV_DECKS } from './fixture'
import type { DevDeck } from './fixture'

/** The address parameters of `/dev/builder`, read leniently: whatever makes no sense is ignored. */
export interface DevBuilderSearch {
  /** Which fixture deck to show; `mixed` when missing or wrong. */
  deck: DevDeck
}

/** Reads `?deck=`; the router has already parsed the query string. */
export function parseDevBuilderSearch(search: Record<string, unknown>): DevBuilderSearch {
  const asked = DEV_DECKS.find((deck) => deck === search.deck)
  return { deck: asked ?? 'mixed' }
}
