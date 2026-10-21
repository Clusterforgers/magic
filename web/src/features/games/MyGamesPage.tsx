import { Link, Navigate } from '@tanstack/react-router'
import { CircleAlertIcon, PlusIcon, RefreshCwIcon, ServerOffIcon, SwordsIcon } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { useFitWindow } from '@/components/appChrome'
import { EmptyState } from '@/components/EmptyState'
import { Loading } from '@/components/Loading'
import { PageShell } from '@/components/PageShell'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button, buttonVariants } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { useAuth, useAuthStore } from '@/features/auth/context'
import { accountErrorText } from '@/features/auth/errors'
import { usePageTitle } from '@/hooks/usePageTitle'
import { cn } from '@/lib/utils'
import type { MyGame } from '@/protocol/generated'
import { gamesText } from './format'
import { GameRows } from './GameRows'

/** How often the "2 minutes ago" texts are brought up to date, in milliseconds. */
export const CLOCK_MS = 60_000

// The shape of the list while it loads: a few rows like the ones that will come.
function RowsSkeleton() {
  return (
    <div className="surface grid gap-3 p-4">
      {[0, 1, 2, 3].map((row) => (
        <Skeleton key={row} className="h-9" />
      ))}
    </div>
  )
}

/** The page's own column: the Play page's, so the two pages of the main navigation share a width and a left edge. */
function Page({ children }: { children: ReactNode }) {
  return (
    <PageShell width="wide" className="flex min-h-0 flex-1 flex-col gap-4 sm:py-6">
      {children}
    </PageShell>
  )
}

/** The title row: the page's name and the way to a new game, as on the Play page. */
function Header() {
  return (
    <header className="flex flex-wrap items-center justify-between gap-3">
      <h1 className="text-2xl font-semibold tracking-tight">My games</h1>
      <Link to="/new" className={cn(buttonVariants({ size: 'lg' }))}>
        <PlusIcon aria-hidden="true" />
        Create game
      </Link>
    </header>
  )
}

/**
 * `/games`: the games the signed-in user holds a seat in, newest first, as a table (stacked rows on a narrow screen)
 * with a way back into the open ones. The page fits the window under the header and only the list scrolls.
 */
export function MyGamesPage({ clock = Date.now }: { clock?: () => number }) {
  usePageTitle('My games')
  useFitWindow()
  const { status, games } = useAuth(useShallow((state) => ({ status: state.status, games: state.games })))
  const store = useAuthStore()
  // The ages are read as the page draws; a render once a minute keeps "2 minutes ago" from going stale.
  const [, setTick] = useState(0)
  // The failed game being taken off the list, and why the last removal did not work.
  const [removing, setRemoving] = useState<string | null>(null)
  const [removeError, setRemoveError] = useState<string | null>(null)
  const now = clock()

  useEffect(() => {
    if (status === 'signed-in') void store.getState().loadGames()
  }, [status, store])
  useEffect(() => {
    const timer = window.setInterval(() => setTick((n) => n + 1), CLOCK_MS)
    return () => window.clearInterval(timer)
  }, [])

  const reload = (): void => void store.getState().loadGames()
  const remove = (game: MyGame): void => {
    setRemoving(game.game)
    setRemoveError(null)
    void store
      .getState()
      .removeGame(game.game)
      .then((result) => {
        setRemoving(null)
        if (!result.ok) setRemoveError(accountErrorText(result.error))
      })
  }

  if (status === 'loading') {
    return (
      <Page>
        <Loading label="Checking your sign-in…">
          <Skeleton className="h-8 w-40" />
        </Loading>
      </Page>
    )
  }
  if (status === 'signed-out') {
    return (
      <Page>
        <Navigate to="/login" search={{ next: '/games' }} replace />
        <p role="status" className="text-sm text-muted-foreground">
          You need to sign in to see your games. Taking you to the sign-in page…
        </p>
      </Page>
    )
  }
  if (status === 'unavailable') {
    return (
      <Page>
        <Header />
        <EmptyState icon={ServerOffIcon} title="No list of games here">
          This server has no accounts, so it keeps no list of games. Open your seat link to get back into a
          game.
        </EmptyState>
      </Page>
    )
  }

  const busy = games.status === 'loading'
  const first = games.status === 'idle' || (busy && games.games.length === 0)
  const failed = games.status === 'failed'
  // A failed reload keeps the rows it had, dimmed, beside the error.
  const stale = failed && games.games.length > 0
  return (
    <Page>
      <Header />

      <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
        {/* A status that is always in the page, so a change of its words is announced. */}
        <p role="status" className="text-sm text-muted-foreground">
          {first || (failed && games.games.length === 0) ? '' : gamesText(games.games.length)}
        </p>
        <Button type="button" variant="outline" size="sm" loading={busy} onClick={reload}>
          {busy ? null : <RefreshCwIcon aria-hidden="true" />}
          Refresh
        </Button>
      </div>

      {failed ? (
        <Alert variant="destructive">
          <CircleAlertIcon aria-hidden="true" />
          <AlertTitle>{stale ? 'Could not refresh your games' : 'Could not load your games'}</AlertTitle>
          <AlertDescription>
            {games.error}
            {stale ? ' The games below may be out of date.' : ''}
          </AlertDescription>
          {/* Under the words, so the icon's column stays as wide as the icon. */}
          <div className="col-start-2 mt-2">
            <Button type="button" loading={busy} onClick={reload}>
              Retry
            </Button>
          </div>
        </Alert>
      ) : null}

      {removeError === null ? null : (
        <Alert variant="destructive">
          <CircleAlertIcon aria-hidden="true" />
          <AlertTitle>Could not remove the game</AlertTitle>
          <AlertDescription>{removeError}</AlertDescription>
        </Alert>
      )}

      {first ? (
        <Loading label="Loading your games…">
          <RowsSkeleton />
        </Loading>
      ) : null}

      {games.status === 'ready' && games.games.length === 0 ? (
        <EmptyState
          icon={SwordsIcon}
          title="You have no games yet"
          action={
            <>
              <Link to="/new" className={buttonVariants()}>
                <PlusIcon aria-hidden="true" />
                Create game
              </Link>
              <Link to="/" className={cn(buttonVariants({ variant: 'outline' }))}>
                Find an open game
              </Link>
            </>
          }
        >
          Games you create while signed in, and games whose seat link you open while signed in, are listed
          here.
        </EmptyState>
      ) : null}

      {games.games.length > 0 ? (
        <div
          role="region"
          aria-label="Your games"
          aria-busy={busy}
          tabIndex={0}
          data-stale={stale || undefined}
          // `relative` so the screen-reader-only words of a row scroll with the list: left to the page they extend it.
          className="surface relative min-h-0 overflow-y-auto overscroll-contain"
        >
          <GameRows games={games.games} now={now} stale={stale} removing={removing} onRemove={remove} />
        </div>
      ) : null}
    </Page>
  )
}
