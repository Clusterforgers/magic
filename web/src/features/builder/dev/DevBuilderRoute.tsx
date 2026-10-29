import { useSearch } from '@tanstack/react-router'
import { cardsApi } from '@/net/cards'
import { useMemo } from 'react'
import { deckView, startDecks } from '@/features/decks/fakeDecks'
import { createDecksApi } from '@/net/decks'
import type { DeckBody } from '@/protocol/generated'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { Builder } from '../Builder'
import { startCards } from '../fakeCards'
import { reportFor, withCheck } from '../fakeCheck'
import { memoryFixMarks } from './memoryFixMarks'
import { BUILDER_FIXTURE_MARKER, fixtureCards, fixtureCheck, fixtureDetails, fixtureView } from './fixture'

/**
 * `/dev/builder?deck=mixed`: the real builder over a fixture deck, a card server and a check endpoint in memory, for
 * looking at it. The decks are `mixed` (a card of each support state and illegal lines), `manual`, `verified`,
 * `changed` and `unchecked`. A card's name opens its preview, the live check answers after a short wait so
 * "Checking…" can be seen, and Save works as if a server answered it: the saved deck is the one checked.
 */
export function DevBuilderRoute() {
  const { deck, cards } = useSearch({ from: '/dev/builder' })
  const world = useMemo(() => {
    const view = fixtureView(deck)
    const pool = fixtureCards()
    const script = fixtureCheck()
    const known = new Map(pool.map((card) => [card.oracle_id, card]))
    const decks = startDecks({
      views: [view],
      pool,
      override: {
        update: (call) => {
          const body: DeckBody = JSON.parse(typeof call.init?.body === 'string' ? call.init.body : 'null')
          const report = reportFor({ deck: null, format: body.format, cards: body.cards }, script)
          const state = report.summary.state
          return jsonResponse(
            200,
            deckView({
              ...view,
              name: body.name,
              format: body.format,
              notes: body.notes,
              cards: body.cards.flatMap((line) => {
                const card = known.get(line.oracle_id)
                return card === undefined
                  ? []
                  : [{ zone: line.zone, count: line.count, printing_id: line.printing_id, card }]
              }),
              state,
              verified: state === 'verified',
              changed: false,
              report,
            }),
          )
        },
      },
    })
    const checks = withCheck(decks.fetch, script)
    return {
      view,
      api: createDecksApi({ fetch: checks.fetch }),
      cards: startCards({ cards: pool, details: fixtureDetails() }),
      fixMarks: memoryFixMarks(pool),
    }
  }, [deck])
  return (
    <>
      <p className="sr-only" data-fixture={BUILDER_FIXTURE_MARKER}>
        Development deck builder with the {deck} deck
      </p>
      <Builder
        key={deck}
        view={world.view}
        api={world.api}
        cards={cards === 'real' ? cardsApi : world.cards.api}
        fixMarks={world.fixMarks}
      />
    </>
  )
}
