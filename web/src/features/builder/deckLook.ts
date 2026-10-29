/** How the deck pane draws the cards, pictures or the dense list, kept in `localStorage` as a convenience of the viewer. */
import { useState } from 'react'

/** Pictures draw each card large in groups by type; the list is one text line a card. */
export type DeckLook = 'pictures' | 'list'

/** The `localStorage` key of the choice. */
export const DECK_LOOK_KEY = 'mtg.deckLook'

/** The stored choice, pictures when none was kept or the storage cannot be read. */
export function loadDeckLook(): DeckLook {
  try {
    return window.localStorage.getItem(DECK_LOOK_KEY) === 'list' ? 'list' : 'pictures'
  } catch {
    return 'pictures'
  }
}

function storeDeckLook(look: DeckLook): void {
  try {
    window.localStorage.setItem(DECK_LOOK_KEY, look)
  } catch {
    // Kept for this page only.
  }
}

/** The look of the deck pane and the setter, which stores the choice. */
export function useDeckLook(): [DeckLook, (look: DeckLook) => void] {
  const [look, setLook] = useState<DeckLook>(loadDeckLook)
  return [
    look,
    (next) => {
      setLook(next)
      storeDeckLook(next)
    },
  ]
}
