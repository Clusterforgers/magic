import { useSearch } from '@tanstack/react-router'
import { useMemo } from 'react'
import { AuthContext } from '@/features/auth/context'
import { createAuthStore, offlineAccounts } from '@/features/auth/store'
import { DecksPage } from '../DecksPage'
import { startDecks } from '../fakeDecks'
import { startGroups } from '../groups/fakeGroups'
import { DECKS_FIXTURE_MARKER, FIXTURE_DECK_LIST, FIXTURE_GROUPS } from './fixture'
import type { DevView } from './decksSearch'

/** The moment the ages count to, so the shots do not change from day to day. */
const DEV_DECKS_NOW = Date.parse('2026-10-18T12:00:00Z')

const never = new Promise<never>(() => undefined)

/** The deck and group servers of a view, in memory: saving, deleting and grouping change them for the page's life. */
function devDecksWorld(view: DevView) {
  const decks = startDecks({
    decks: view === 'empty' ? [] : [...FIXTURE_DECK_LIST],
    unavailable: view === 'unavailable',
    override: {
      ...(view === 'loading' ? { list: () => never } : {}),
      ...(view === 'error' ? { list: () => Promise.reject(new TypeError('Failed to fetch')) } : {}),
    },
  })
  const groups = startGroups(view === 'groups' ? [...FIXTURE_GROUPS] : [])
  return { decks: decks.api, groups: groups.api }
}

/**
 * `/dev/decks?view=groups`: the real Decks page signed in as Ada over decks and groups in memory. `view` is `groups`
 * (three groups, one folded, a deck in two and one in none), `list` (the same decks without groups), `empty`,
 * `loading`, `error` or `unavailable` (a server without decks).
 */
export function DevDecksRoute() {
  const { view } = useSearch({ from: '/dev/decks' })
  const world = useMemo(() => devDecksWorld(view), [view])
  const auth = useMemo(
    () =>
      createAuthStore({
        api: { ...offlineAccounts, myGames: async () => [] },
        initial: { status: 'signed-in', user: { id: 'dev-ada', username: 'Ada_L', is_admin: false } },
      }),
    [],
  )
  return (
    <AuthContext value={auth}>
      <p className="sr-only" data-fixture={DECKS_FIXTURE_MARKER}>
        Development list of decks
      </p>
      <DecksPage key={view} api={world.decks} groupsApi={world.groups} clock={() => DEV_DECKS_NOW} />
    </AuthContext>
  )
}
