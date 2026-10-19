import { useSearch } from '@tanstack/react-router'
import { useMemo } from 'react'
import { startDecks } from '@/features/decks/fakeDecks'
import { Builder } from '../Builder'
import { startCards } from '../fakeCards'
import { BUILDER_FIXTURE_MARKER, fixtureCards, fixtureView } from './fixture'

/**
 * `/dev/builder?deck=mixed`: the real builder over a fixture deck and a card server in memory, for looking at it. The
 * decks are `mixed` (a card of each support state and illegal lines), `manual`, `verified`, `changed` and `unchecked`.
 * Save works as if a server answered it.
 */
export function DevBuilderRoute() {
  const { deck } = useSearch({ from: '/dev/builder' })
  const world = useMemo(() => {
    const view = fixtureView(deck)
    const pool = fixtureCards()
    return { view, decks: startDecks({ views: [view], pool }), cards: startCards({ cards: pool }) }
  }, [deck])
  return (
    <>
      <p className="sr-only" data-fixture={BUILDER_FIXTURE_MARKER}>
        Development deck builder with the {deck} deck
      </p>
      <Builder key={deck} view={world.view} api={world.decks.api} cards={world.cards.api} />
    </>
  )
}
