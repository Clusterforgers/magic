import { useParams } from '@tanstack/react-router'
import { BuilderPage } from './BuilderPage'

/** The page for the address `/decks/$deckId`: the builder of the deck that the address names. */
export function BuilderRoute() {
  const { deckId } = useParams({ from: '/decks_/$deckId' })
  return <BuilderPage deckId={deckId} />
}
