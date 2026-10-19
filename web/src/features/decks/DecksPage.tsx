import { Link, Navigate } from '@tanstack/react-router'
import { Loading } from '@/components/Loading'
import { PageShell } from '@/components/PageShell'
import { buttonVariants } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { useAuth } from '@/features/auth/context'
import { usePageTitle } from '@/hooks/usePageTitle'
import { decksApi } from '@/net/decks'
import type { DecksApi } from '@/net/decks'
import { DeckLibrary } from './DeckLibrary'

interface DecksPageProps {
  /** The deck endpoints; the shared client unless a test brings its own. */
  api?: DecksApi
  /** The time the ages are counted to; the real clock unless a test fixes it. */
  clock?: () => number
}

/**
 * `/decks`: the signed-in user's decks. Someone who is not signed in is sent to the sign-in form and back; a server
 * without accounts has no decks to show.
 */
export function DecksPage({ api = decksApi, clock = Date.now }: DecksPageProps) {
  usePageTitle('Decks')
  const status = useAuth((state) => state.status)

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
        <Navigate to="/login" search={{ next: '/decks' }} replace />
        <p role="status" className="text-sm text-muted-foreground">
          You need to sign in to see your decks. Taking you to the sign-in page…
        </p>
      </PageShell>
    )
  }
  if (status === 'unavailable') {
    return (
      <PageShell>
        <section aria-labelledby="decks-title" className="grid justify-items-start gap-3">
          <h1 id="decks-title" className="text-2xl font-semibold tracking-tight">
            Decks
          </h1>
          <p className="text-base text-muted-foreground">
            This server has no accounts, so it keeps no decks. Pick one of its built-in decks when you create
            a game.
          </p>
          <Link to="/" className={buttonVariants({ size: 'lg' })}>
            Create a game
          </Link>
        </section>
      </PageShell>
    )
  }
  return <DeckLibrary api={api} clock={clock} />
}
