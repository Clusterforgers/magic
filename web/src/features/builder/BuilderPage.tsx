import { Link, Navigate } from '@tanstack/react-router'
import { CircleAlertIcon, LibraryIcon, TriangleAlertIcon } from 'lucide-react'
import { EmptyState } from '@/components/EmptyState'
import { Loading } from '@/components/Loading'
import { PageShell } from '@/components/PageShell'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button, buttonVariants } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { useAuth } from '@/features/auth/context'
import { usePageTitle } from '@/hooks/usePageTitle'
import { cardsApi } from '@/net/cards'
import type { CardsApi } from '@/net/cards'
import { decksApi } from '@/net/decks'
import type { DecksApi } from '@/net/decks'
import type { FixMarksApi } from '@/net/fixMarks'
import { Builder } from './Builder'
import { useDeckView } from './useDeckView'

interface BuilderPageProps {
  /** The id of the deck to edit, from the address. */
  deckId: string
  /** The deck endpoints; the shared client unless a test brings its own. */
  api?: DecksApi
  /** The card endpoints; the shared client unless a test brings its own. */
  cards?: CardsApi
  /** The fix-mark endpoints; with them a card's preview offers "Mark important to fix". */
  fixMarks?: FixMarksApi
}

// The two panes while the deck loads.
function PanesSkeleton() {
  return (
    <div className="grid gap-4 lg:grid-cols-2">
      <Skeleton className="h-96 rounded-xl" />
      <Skeleton className="h-96 rounded-xl" />
    </div>
  )
}

function DeckLoader({
  deckId,
  api,
  cards,
  fixMarks,
}: Required<Omit<BuilderPageProps, 'fixMarks'>> & Pick<BuilderPageProps, 'fixMarks'>) {
  const { state, reload } = useDeckView(api, deckId)

  if (state.status === 'ready')
    return <Builder key={state.view.id} view={state.view} api={api} cards={cards} fixMarks={fixMarks} />
  return (
    <PageShell width="wide">
      {state.status === 'loading' ? (
        <Loading label="Loading the deck…">
          <PanesSkeleton />
        </Loading>
      ) : null}

      {state.status === 'missing' ? (
        <EmptyState
          icon={LibraryIcon}
          as="h1"
          title="That deck was not found"
          action={
            <Link to="/decks" className={buttonVariants()}>
              Back to your decks
            </Link>
          }
        >
          It may have been deleted, or it belongs to another account.
        </EmptyState>
      ) : null}

      {state.status === 'unavailable' ? (
        <Alert variant="warning" role="status">
          <TriangleAlertIcon aria-hidden="true" />
          <AlertTitle>This server keeps no decks</AlertTitle>
          <AlertDescription>
            It runs without a database, so decks cannot be opened or saved. You can still play with its
            built-in decks.
          </AlertDescription>
          <div className="mt-2">
            <Link to="/" className={buttonVariants({ variant: 'outline' })}>
              Create a game
            </Link>
          </div>
        </Alert>
      ) : null}

      {state.status === 'failed' ? (
        <Alert variant="destructive">
          <CircleAlertIcon aria-hidden="true" />
          <AlertTitle>Could not load the deck</AlertTitle>
          <AlertDescription>{state.message}</AlertDescription>
          <div className="mt-2">
            {state.sessionEnded ? (
              <Link to="/login" search={{ next: `/decks/${deckId}` }} className={buttonVariants()}>
                Sign in
              </Link>
            ) : (
              <Button type="button" onClick={reload}>
                Retry
              </Button>
            )}
          </div>
        </Alert>
      ) : null}
    </PageShell>
  )
}

/**
 * `/decks/$deckId`: the builder for one of the signed-in user's decks. Someone who is not signed in is sent to the
 * sign-in form and back; a server without accounts keeps no decks; a deck that is not the account's reads as missing.
 */
export function BuilderPage({ deckId, api = decksApi, cards = cardsApi, fixMarks }: BuilderPageProps) {
  usePageTitle('Deck builder')
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
        <Navigate to="/login" search={{ next: `/decks/${deckId}` }} replace />
        <p role="status" className="text-sm text-muted-foreground">
          You need to sign in to edit a deck. Taking you to the sign-in page…
        </p>
      </PageShell>
    )
  }
  if (status === 'unavailable') {
    return (
      <PageShell>
        <section aria-labelledby="builder-title" className="grid justify-items-start gap-3">
          <h1 id="builder-title" className="text-2xl font-semibold tracking-tight">
            Deck builder
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
  return <DeckLoader deckId={deckId} api={api} cards={cards} fixMarks={fixMarks} />
}
