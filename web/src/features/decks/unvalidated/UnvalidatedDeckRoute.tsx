import { getRouteApi } from '@tanstack/react-router'
import { fixMarksApi } from '@/net/fixMarks'
import { UnvalidatedPage } from './UnvalidatedPage'

const route = getRouteApi('/decks_/$deckId_/unvalidated')

/** `/decks/$deckId/unvalidated`: the page for the deck the address names. */
export function UnvalidatedDeckRoute() {
  const { deckId } = route.useParams()
  return <UnvalidatedPage deckId={deckId} fixMarks={fixMarksApi} />
}
