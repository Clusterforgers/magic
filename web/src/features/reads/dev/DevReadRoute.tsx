import { useSearch } from '@tanstack/react-router'
import { useMemo, useState } from 'react'
import { PageShell } from '@/components/PageShell'
import { Button } from '@/components/ui/button'
import { AuthContext } from '@/features/auth/context'
import { createAuthStore, offlineAccounts } from '@/features/auth/store'
import { ReadsApiContext } from '../apiContext'
import { ReadDialog } from '../ReadDialog'
import { ReadPage } from '../ReadPage'
import { READ_CARDS, READS_FIXTURE_MARKER, fixtureReadsApi } from './fixture'
import type { ReadFixtureOptions } from './fixture'
import type { DevReadView } from './readSearch'

function optionsOf(view: DevReadView): ReadFixtureOptions {
  switch (view) {
    case 'done':
      return { empty: true }
    case 'loading':
      return { loading: true }
    case 'error':
      return { listFails: 'network' }
    case 'stale':
      return { stale: true }
    default:
      return { delayMs: 150 }
  }
}

const FIRST = Object.values(READ_CARDS)[0]

/** The table's way in: a button that opens `ReadDialog` on the first fixture card, open from the start. */
function DialogScene() {
  const [open, setOpen] = useState(true)
  return (
    <PageShell width="wide">
      <Button onClick={() => setOpen(true)}>Read {FIRST?.name}</Button>
      <ReadDialog
        oracleId={open ? (FIRST?.oracle_id ?? null) : null}
        name={FIRST?.name ?? ''}
        onClose={() => setOpen(false)}
      />
    </PageShell>
  )
}

/**
 * `/dev/read?view=deck`: the real review screen over made-up cards in memory, signed in. `view` is `deck`, `precon`, `all`,
 * `done` (nothing left to read), `loading`, `error`, `stale` (every verdict meets a changed draft) or `dialog` (the
 * table's `ReadDialog`).
 */
export function DevReadRoute() {
  const { view } = useSearch({ from: '/dev/read' })
  const api = useMemo(() => fixtureReadsApi(optionsOf(view)), [view])
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
      <ReadsApiContext value={api}>
        <p className="sr-only" data-fixture={READS_FIXTURE_MARKER}>
          Development review screen
        </p>
        {view === 'dialog' ? (
          <DialogScene />
        ) : (
          <ReadPage
            key={view}
            api={api}
            target={
              view === 'mine'
                ? { mine: true }
                : view === 'all'
                  ? null
                  : view === 'precon'
                    ? { precon: 'c18-adaptive-enchantment' }
                    : { deck: 'deck-izzet' }
            }
          />
        )}
      </ReadsApiContext>
    </AuthContext>
  )
}
