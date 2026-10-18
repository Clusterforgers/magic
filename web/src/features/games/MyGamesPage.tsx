import { Link, Navigate } from '@tanstack/react-router'
import { LoaderCircle, RefreshCw } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button, buttonVariants } from '@/components/ui/button'
import { useAuth, useAuthStore } from '@/features/auth/context'
import { usePageTitle } from '@/hooks/usePageTitle'
import { GameRow } from './GameRow'

/** How often the "2 minutes ago" texts are brought up to date, in milliseconds. */
export const CLOCK_MS = 60_000

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
      <Shell>
        <p role="status" className="text-sm text-muted-foreground">
          Checking your sign-in…
        </p>
      </Shell>
    )
  }
  if (status === 'signed-out') {
    return (
      <Shell>
        <Navigate to="/login" search={{ next: '/games' }} replace />
        <p role="status" className="text-sm text-muted-foreground">
          You need to sign in to see your games. Taking you to the sign-in page…
        </p>
      </Shell>
    )
  }
  if (status === 'unavailable') {
    return (
      <Shell>
        <section aria-labelledby="games-title" className="grid justify-items-start gap-3">
          <h1 id="games-title" className="text-2xl font-semibold tracking-tight">
            My games
          </h1>
          <p className="text-sm text-muted-foreground">
            This server has no accounts, so it keeps no list of games. Open your seat link to get back into a
            game.
          </p>
          <Link to="/" className={buttonVariants({ size: 'lg' })}>
            Create a game
          </Link>
        </section>
      </Shell>
    )
  }

  const busy = games.status === 'loading'
  const first = games.status === 'idle' || (busy && games.games.length === 0)
  return (
    <Shell>
      <section aria-labelledby="games-title" className="grid gap-5">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <h1 id="games-title" className="text-2xl font-semibold tracking-tight">
            My games
          </h1>
          <div className="flex items-center gap-2">
            <Button type="button" variant="outline" disabled={busy} onClick={reload}>
              {busy ? (
                <LoaderCircle aria-hidden="true" className="animate-spin motion-reduce:animate-none" />
              ) : (
                <RefreshCw aria-hidden="true" />
              )}
              {busy ? 'Reloading…' : 'Reload'}
            </Button>
            <Link to="/" className={buttonVariants()}>
              New game
            </Link>
          </div>
        </div>

        {first ? (
          <p role="status" className="text-sm text-muted-foreground">
            Loading your games…
          </p>
        ) : null}

        {games.status === 'failed' ? (
          <Alert variant="destructive" className="gap-2 p-4">
            <AlertTitle>Could not load your games</AlertTitle>
            <AlertDescription className="text-destructive">{games.error}</AlertDescription>
            <div>
              <Button type="button" onClick={reload}>
                Try again
              </Button>
            </div>
          </Alert>
        ) : null}

        {games.status === 'ready' && games.games.length === 0 ? (
          <div className="grid justify-items-start gap-3 rounded-xl bg-card p-4 ring-1 ring-foreground/10">
            <h2 className="font-medium">You have no games yet</h2>
            <p className="text-sm text-muted-foreground">
              Games you create while signed in, and games whose seat link you open while signed in, are listed
              here.
            </p>
            <Link to="/" className={buttonVariants({ size: 'lg' })}>
              Create a game
            </Link>
          </div>
        ) : null}

        {games.games.length > 0 ? (
          <ul aria-label="Your games" aria-busy={busy} className="grid gap-3">
            {games.games.map((game) => (
              <GameRow key={game.game} game={game} now={now} />
            ))}
          </ul>
        ) : null}
      </section>
    </Shell>
  )
}

function Shell({ children }: { children: ReactNode }) {
  return <div className="mx-auto w-full max-w-3xl px-4 py-8 sm:px-6">{children}</div>
}
