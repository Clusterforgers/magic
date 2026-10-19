import { Link, Navigate } from '@tanstack/react-router'
import { CircleAlertIcon, PlusIcon, RefreshCwIcon, SwordsIcon } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { EmptyState } from '@/components/EmptyState'
import { Loading } from '@/components/Loading'
import { PageShell } from '@/components/PageShell'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button, buttonVariants } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { useAuth, useAuthStore } from '@/features/auth/context'
import { usePageTitle } from '@/hooks/usePageTitle'
import { GameRow } from './GameRow'

/** How often the "2 minutes ago" texts are brought up to date, in milliseconds. */
export const CLOCK_MS = 60_000

// The shape of the list while it loads: three rows like the ones that will come.
function RowsSkeleton() {
  return (
    <div className="grid gap-3">
      {[0, 1, 2].map((row) => (
        <Skeleton key={row} className="h-24 rounded-xl" />
      ))}
    </div>
  )
}

/** `/games`: the games the signed-in user holds a seat in, newest first, with a way back into the open ones. */
export function MyGamesPage({ clock = Date.now }: { clock?: () => number }) {
  usePageTitle('My games')
  const { status, games } = useAuth(useShallow((state) => ({ status: state.status, games: state.games })))
  const store = useAuthStore()
  // The ages are read as the page draws; a render once a minute keeps "2 minutes ago" from going stale.
  const [, setTick] = useState(0)
  const now = clock()

  useEffect(() => {
    if (status === 'signed-in') void store.getState().loadGames()
  }, [status, store])
  useEffect(() => {
    const timer = window.setInterval(() => setTick((n) => n + 1), CLOCK_MS)
    return () => window.clearInterval(timer)
  }, [])

  const reload = (): void => void store.getState().loadGames()

  if (status === 'loading') {
    return (
      <PageShell>
        <Loading label="Checking your sign-in…">
          <Skeleton className="h-8 w-40" />
        </Loading>
      </PageShell>
    )
  }
  if (status === 'signed-out') {
    return (
      <PageShell>
        <Navigate to="/login" search={{ next: '/games' }} replace />
        <p role="status" className="text-sm text-muted-foreground">
          You need to sign in to see your games. Taking you to the sign-in page…
        </p>
      </PageShell>
    )
  }
  if (status === 'unavailable') {
    return (
      <PageShell>
        <section aria-labelledby="games-title" className="grid justify-items-start gap-3">
          <h1 id="games-title" className="text-2xl font-semibold tracking-tight">
            My games
          </h1>
          <p className="text-base text-muted-foreground">
            This server has no accounts, so it keeps no list of games. Open your seat link to get back into a
            game.
          </p>
          <Link to="/new" className={buttonVariants({ size: 'lg' })}>
            Create a game
          </Link>
        </section>
      </PageShell>
    )
  }

  const busy = games.status === 'loading'
  const first = games.status === 'idle' || (busy && games.games.length === 0)
  return (
    <PageShell>
      <section aria-labelledby="games-title" className="grid gap-6">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <h1 id="games-title" className="text-2xl font-semibold tracking-tight">
            My games
          </h1>
          <div className="flex items-center gap-2">
            <Button type="button" variant="outline" loading={busy} onClick={reload}>
              {busy ? null : <RefreshCwIcon aria-hidden="true" />}
              {busy ? 'Reloading…' : 'Reload'}
            </Button>
            <Link to="/new" className={buttonVariants()}>
              <PlusIcon aria-hidden="true" />
              New game
            </Link>
          </div>
        </div>

        {first ? (
          <Loading label="Loading your games…">
            <RowsSkeleton />
          </Loading>
        ) : null}

        {games.status === 'failed' ? (
          <Alert variant="destructive">
            <CircleAlertIcon aria-hidden="true" />
            <AlertTitle>Could not load your games</AlertTitle>
            <AlertDescription>{games.error}</AlertDescription>
            <div className="mt-2">
              <Button type="button" onClick={reload}>
                Retry
              </Button>
            </div>
          </Alert>
        ) : null}

        {games.status === 'ready' && games.games.length === 0 ? (
          <EmptyState
            icon={SwordsIcon}
            title="You have no games yet"
            action={
              <Link to="/new" className={buttonVariants({ size: 'lg' })}>
                Create a game
              </Link>
            }
          >
            Games you create while signed in, and games whose seat link you open while signed in, are listed
            here.
          </EmptyState>
        ) : null}

        {games.games.length > 0 ? (
          <ul aria-label="Your games" aria-busy={busy} className="grid gap-3">
            {games.games.map((game) => (
              <GameRow key={game.game} game={game} now={now} />
            ))}
          </ul>
        ) : null}
      </section>
    </PageShell>
  )
}
