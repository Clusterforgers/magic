/** Finds the picture of a card a delve offers by name: its oracle id through the card search, once per name. */
import { cardsApi } from '@/net/cards'
import type { CardsApi } from '@/net/cards'

/** Finds a card's oracle id by its exact name; null where the search has none. */
export type FindCard = (name: string) => Promise<string | null>

const found = new Map<string, Promise<string | null>>()

/** Finds a card through `GET /api/cards`, once per name for the page's life. */
export function searchByName(api: Pick<CardsApi, 'search'> = cardsApi): FindCard {
  return (name) => {
    const known = found.get(name)
    if (known !== undefined) return known
    const ask = api
      .search({ text: name, perPage: 10 })
      .then((page) => page.cards.find((card) => card.name === name)?.oracle_id ?? null)
      .catch(() => null)
    found.set(name, ask)
    return ask
  }
}
