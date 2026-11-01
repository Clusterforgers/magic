/** The grid a prompt's cards sit in, which the preview of any of them docks away from. */
import { createContext, useContext } from 'react'
import type { RefObject } from 'react'

/** The element the cards of a choice dock their preview from; none outside a `CardChoices`, where a card docks from itself. */
export const ChoiceAnchorContext = createContext<RefObject<HTMLElement | null> | null>(null)

/** For `usePreviewHandlers`: the grid of the choice around the caller, else the card itself. */
export function useChoiceAnchor(): (card: HTMLElement) => Element {
  const grid = useContext(ChoiceAnchorContext)
  return (card) => grid?.current ?? card
}
