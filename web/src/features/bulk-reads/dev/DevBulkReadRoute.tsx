import { useMemo } from 'react'
import { AuthContext } from '@/features/auth/context'
import { createAuthStore, offlineAccounts } from '@/features/auth/store'
import { BulkReadPage } from '../BulkReadPage'
import { BULK_READS_FIXTURE_MARKER, fixtureBulkReadsApi } from './fixture'

/** `/dev/read-bulk`: the real bulk read screen over made-up cards in memory, signed in. */
export function DevBulkReadRoute() {
  const api = useMemo(() => fixtureBulkReadsApi(), [])
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
      <BulkReadPage api={api} />
      <p className="sr-only" data-fixture={BULK_READS_FIXTURE_MARKER}>
        Made-up cards in memory
      </p>
    </AuthContext>
  )
}
