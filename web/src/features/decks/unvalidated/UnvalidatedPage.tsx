import { Link, Navigate } from '@tanstack/react-router'
import { Loading } from '@/components/Loading'
import { PageShell } from '@/components/PageShell'
import { buttonVariants } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { useAuth } from '@/features/auth/context'
import { usePageTitle } from '@/hooks/usePageTitle'
import { cardsApi } from '@/net/cards'
import type { CardsApi } from '@/net/cards'
import { decksApi } from '@/net/decks'
import type { DecksApi } from '@/net/decks'
import type { FixMarksApi } from '@/net/fixMarks'
import { UnvalidatedView } from './UnvalidatedView'

interface UnvalidatedPageProps {
  /** One deck's id for `/decks/$deckId/unvalidated`; every saved deck for `/decks/unvalidated`. */
  deckId?: string
  /** The deck endpoints; the shared client unless a test brings its own. */
  api?: Pick<DecksApi, 'list' | 'get' | 'check'>
  /** The card endpoint the clauses are quoted from. */
  cards?: Pick<CardsApi, 'get'>
  /** The fix-mark endpoints; with them each card gets a flag that marks it important to fix. */
  fixMarks?: FixMarksApi
}

/**
 * `/decks/unvalidated` and `/decks/$deckId/unvalidated`: the cards the game does not run whole, for the signed-in
 * user's decks. Someone who is not signed in is sent to the sign-in form and back; a server without accounts has no
 * decks to check.
 */
export function UnvalidatedPage({
  deckId,
  api = decksApi,
  cards = cardsApi,
  fixMarks,
}: UnvalidatedPageProps) {
  usePageTitle('Not validated cards')
  const status = useAuth((state) => state.status)
  const path = deckId === undefined ? '/decks/unvalidated' : `/decks/${deckId}/unvalidated`

  if (status === 'loading') {
    return (
      <PageShell width="wide">
        <Loading label="Checking your sign-in…">
          <Skeleton className="h-8 w-40" />
        </Loading>
      </PageShell>
    )
  }
  if (status === 'signed-out') {
    return (
      <PageShell width="wide">
        <Navigate to="/login" search={{ next: path }} replace />
        <p role="status" className="text-sm text-muted-foreground">
          You need to sign in to check your decks. Taking you to the sign-in page…
        </p>
      </PageShell>
    )
  }
  if (status === 'unavailable') {
    return (
      <PageShell width="wide">
        <section aria-labelledby="unvalidated-title" className="grid justify-items-start gap-3">
          <h1 id="unvalidated-title" className="text-2xl font-semibold tracking-tight">
            Not validated cards
          </h1>
          <p className="text-base text-muted-foreground">
            This server has no accounts, so it keeps no decks to check. Pick one of its built-in decks when
            you create a game.
          </p>
          <Link to="/" className={buttonVariants({ size: 'lg' })}>
            Create a game
          </Link>
        </section>
      </PageShell>
    )
  }
  return (
    <PageShell width="wide">
      <UnvalidatedView
        key={deckId ?? '*'}
        api={api}
        cards={cards}
        fixMarks={fixMarks}
        scope={deckId === undefined ? { kind: 'all' } : { kind: 'deck', id: deckId }}
      />
    </PageShell>
  )
}
