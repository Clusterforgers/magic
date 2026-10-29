import { useSearch } from '@tanstack/react-router'
import { useMemo } from 'react'
import { AuthContext } from '@/features/auth/context'
import { createAuthStore, offlineAccounts } from '@/features/auth/store'
import { FIXTURE_DECKS, fixtureCardsApi, fixtureDecksApi } from '../fixture'
import type { FixtureOptions } from '../fixture'
import { UnvalidatedPage } from '../UnvalidatedPage'
import type { DevView } from './unvalidatedSearch'

/** Text the production build must not contain (`devBuild.test.ts` looks for it). */
export const UNVALIDATED_FIXTURE_MARKER = 'unvalidated-fixture'

const never = new Promise<never>(() => undefined)

function optionsOf(view: DevView): FixtureOptions {
  switch (view) {
    case 'clean':
      return { decks: FIXTURE_DECKS.filter((deck) => deck.item.id === 'deck-burn') }
    case 'none':
      return { decks: [] }
    case 'error':
      return { listFails: 'network' }
    case 'unavailable':
      return { listFails: 'decks_unavailable' }
    case 'failed':
      return { checkFails: ['deck-izzet'] }
    case 'texts':
      return { cardsFail: true }
    default:
      return { delayMs: 250 }
  }
}

/**
 * `/dev/unvalidated?view=all`: the real page over four fixture decks in memory, for looking at it. `view` is `all`
 * (every deck, merged by card), `deck` (the page of one deck), `clean` (a deck that runs whole), `none` (no decks),
 * `loading` (the list never arrives), `error`, `unavailable`, `failed` (one deck's check fails) or `texts` (the card
 * texts cannot be read, so the clauses show without quotes).
 */
export function DevUnvalidatedRoute() {
  const { view } = useSearch({ from: '/dev/unvalidated' })
  const world = useMemo(() => {
    const decks = fixtureDecksApi(optionsOf(view))
    return {
      decks: view === 'loading' ? { ...decks, list: () => never } : decks,
      cards: fixtureCardsApi(optionsOf(view)),
    }
  }, [view])
  const auth = useMemo(
    () =>
      createAuthStore({
        api: { ...offlineAccounts, myGames: async () => [] },
        initial: { status: 'signed-in', user: { id: 'dev-ada', username: 'Ada_L' } },
      }),
    [],
  )
  return (
    <AuthContext value={auth}>
      <p className="sr-only" data-fixture={UNVALIDATED_FIXTURE_MARKER}>
        Development list of cards that are not validated
      </p>
      <UnvalidatedPage
        key={view}
        {...(view === 'deck' ? { deckId: 'deck-izzet' } : {})}
        api={world.decks}
        cards={world.cards}
      />
    </AuthContext>
  )
}
