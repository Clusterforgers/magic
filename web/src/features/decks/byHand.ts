/** Marking a card of a saved deck "play by hand": the body that saves the deck with the card marked, and the save. */
import type { DecksApi } from '@/net/decks'
import type { DeckBody, DeckView } from '@/protocol/generated'

/** The deck endpoints marking a card "play by hand" needs. */
export type ByHandApi = Pick<DecksApi, 'get' | 'update'>

/** A deck of the signed-in owner that holds a card, and whether the owner already plays the card by hand in it. */
export interface ByHandDeck {
  id: string
  name: string
  byHand: boolean
}

/** The body that saves `view` with every line of `oracleId` marked "play by hand", the other lines as they are. */
export function byHandBody(
  view: Pick<DeckView, 'name' | 'format' | 'notes' | 'cards'>,
  oracleId: string,
): DeckBody {
  return {
    name: view.name,
    format: view.format,
    notes: view.notes,
    cards: view.cards.map((line) => ({
      zone: line.zone,
      oracle_id: line.card.oracle_id,
      printing_id: line.printing_id,
      count: line.count,
      ...(line.by_hand === true || line.card.oracle_id === oracleId ? { by_hand: true } : {}),
    })),
  }
}

/** Saves deck `deckId` with `oracleId` played by hand: the deck as stored now, the one card marked. */
export async function playByHand(api: ByHandApi, deckId: string, oracleId: string): Promise<DeckView> {
  const view = await api.get(deckId)
  return api.update(deckId, byHandBody(view, oracleId))
}
