import { useSearch } from '@tanstack/react-router'
import { useMemo } from 'react'
import { AuthContext } from '@/features/auth/context'
import { createAuthStore, offlineAccounts } from '@/features/auth/store'
import { DeckError } from '@/net/decks'
import type { CardSupportApi } from '@/net/cardSupport'
import { CardSupportPage } from '../CardSupportPage'
import { FIXTURE_MISSING, FIXTURE_OVERVIEW, SUPPORT_FIXTURE_MARKER } from './fixture'
import type { DevSupportView } from './supportSearch'

/** The fixture's answers for `view`. */
export function devSupportApi(view: DevSupportView): CardSupportApi {
  switch (view) {
    case 'empty':
      return {
        overview: async () => ({ snapshot: null, history: [], last_check: null, feed: [] }),
        missing: async () => ({ snapshot_id: null, cards: [] }),
      }
    case 'loading':
      return { overview: () => new Promise(() => undefined), missing: () => new Promise(() => undefined) }
    case 'error':
      return {
        overview: async () => {
          throw new DeckError(
            'unavailable',
            503,
            'The card database is not available right now; try again later.',
          )
        },
        missing: async () => FIXTURE_MISSING,
      }
    case 'full':
      return { overview: async () => FIXTURE_OVERVIEW, missing: async () => FIXTURE_MISSING }
  }
}

/** `/dev/support?view=full|empty|loading|error`: the real Card support page, signed in, over the fixture. */
export function DevSupportRoute() {
  const { view } = useSearch({ from: '/dev/support' })
  const api = useMemo(() => devSupportApi(view), [view])
  const auth = useMemo(
    () =>
      createAuthStore({
        api: offlineAccounts,
        initial: { status: 'signed-in', user: { id: 'dev-0', username: 'Dev', is_admin: false } },
      }),
    [],
  )
  return (
    <AuthContext value={auth}>
      <p className="sr-only" data-fixture={SUPPORT_FIXTURE_MARKER}>
        Made-up card support numbers.
      </p>
      <CardSupportPage api={api} />
    </AuthContext>
  )
}
