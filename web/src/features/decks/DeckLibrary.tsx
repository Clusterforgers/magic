import { Link, useNavigate } from '@tanstack/react-router'
import {
  CircleAlertIcon,
  LibraryIcon,
  PlusIcon,
  ShieldHalfIcon,
  TriangleAlertIcon,
  UploadIcon,
} from 'lucide-react'
import { useRef, useState } from 'react'
import { EmptyState } from '@/components/EmptyState'
import { Loading } from '@/components/Loading'
import { PageShell } from '@/components/PageShell'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button, buttonVariants } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import type { DecksApi } from '@/net/decks'
import type { DeckListItem, DeckView } from '@/protocol/generated'
import { DeckRow } from './DeckRow'
import { DeleteDeckDialog } from './DeleteDeckDialog'
import { ImportDialog } from './ImportDialog'
import { NewDeckDialog } from './NewDeckDialog'
import { useDeckList } from './useDeckList'

type Overlay =
  | { key: number; kind: 'new' }
  | { key: number; kind: 'import' }
  | { key: number; kind: 'delete'; deck: DeckListItem }

// The shape of the list while it loads: rows like the ones that will come.
function RowsSkeleton() {
  return (
    <div className="grid gap-3">
      {[0, 1, 2].map((row) => (
        <Skeleton key={row} className="h-20 rounded-xl" />
      ))}
    </div>
  )
}

interface DeckLibraryProps {
  api: DecksApi
  clock: () => number
}

/**
 * The signed-in user's decks: the list with each deck's check state, and New deck, Import and Delete. From the `sm`
 * breakpoint up the page is as tall as the window under the header and the list scrolls inside its own region.
 */
export function DeckLibrary({ api, clock }: DeckLibraryProps) {
  const { state, reload } = useDeckList(api)
  const [overlay, setOverlay] = useState<Overlay | null>(null)
  const [announcement, setAnnouncement] = useState('')
  const opened = useRef(0)
  const titleRef = useRef<HTMLHeadingElement>(null)
  const headingNext = useRef(false)
  const navigate = useNavigate()

  const open = (
    next: { kind: 'new' } | { kind: 'import' } | { kind: 'delete'; deck: DeckListItem },
  ): void => {
    opened.current += 1
    setOverlay({ ...next, key: opened.current })
  }
  // A dialog that finishes closing after another one opened must not take the new one down with it.
  const closed = (key: number): void => {
    setOverlay((current) => (current?.key === key ? null : current))
    // The row that had the focus is gone; the page heading is where the list begins. The dialog hands the focus back
    // first, so the heading takes it once the dialog has closed.
    if (headingNext.current) {
      headingNext.current = false
      titleRef.current?.focus()
    }
  }

  // A new or imported deck opens in the builder, where its cards and its check are.
  const saved = (deck: DeckView): void => {
    void navigate({ to: '/decks/$deckId', params: { deckId: deck.id } })
  }
  const deleted = (deck: DeckListItem): void => {
    setAnnouncement(`Deleted “${deck.name}”.`)
    headingNext.current = true
    reload()
  }

  const decks = state.status === 'ready' ? state.decks : []
  const now = clock()
  const importAction = (
    <Button type="button" aria-haspopup="dialog" onClick={() => open({ kind: 'import' })}>
      <UploadIcon aria-hidden="true" />
      Import deck
    </Button>
  )
  const newAction = (
    <Button type="button" variant="outline" aria-haspopup="dialog" onClick={() => open({ kind: 'new' })}>
      <PlusIcon aria-hidden="true" />
      New deck
    </Button>
  )

  return (
    <PageShell width="wide" className="sm:flex sm:h-[calc(100dvh-3.5rem-1px)] sm:flex-col">
      <section aria-labelledby="decks-title" className="flex min-h-0 flex-1 flex-col gap-6">
        <div className="flex flex-wrap items-end justify-between gap-3">
          <div className="grid gap-1">
            <h1
              id="decks-title"
              ref={titleRef}
              tabIndex={-1}
              className="text-2xl font-semibold tracking-tight"
            >
              Decks
            </h1>
            {state.status === 'ready' ? (
              <p className="text-sm text-muted-foreground">
                {decks.length} {decks.length === 1 ? 'deck' : 'decks'}
              </p>
            ) : null}
          </div>
          {state.status === 'unavailable' ? null : (
            <div className="flex flex-wrap items-center gap-2">
              <Link to="/decks/unvalidated" className={buttonVariants({ variant: 'ghost' })}>
                <ShieldHalfIcon aria-hidden="true" />
                Not validated cards
              </Link>
              {newAction}
              {importAction}
            </div>
          )}
        </div>
        <p role="status" className="sr-only">
          {announcement}
        </p>

        {state.status === 'loading' ? (
          <Loading label="Loading your decks…">
            <RowsSkeleton />
          </Loading>
        ) : null}

        {state.status === 'failed' ? (
          <Alert variant="destructive">
            <CircleAlertIcon aria-hidden="true" />
            <AlertTitle>Could not load your decks</AlertTitle>
            <AlertDescription>{state.message}</AlertDescription>
            <div className="mt-2">
              {state.sessionEnded ? (
                <Link to="/login" search={{ next: '/decks' }} className={buttonVariants()}>
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

        {state.status === 'unavailable' ? (
          <Alert variant="warning" role="status">
            <TriangleAlertIcon aria-hidden="true" />
            <AlertTitle>This server keeps no decks</AlertTitle>
            <AlertDescription>
              It runs without a database, so decks cannot be saved or listed. You can still play with its
              built-in decks.
            </AlertDescription>
            <div className="mt-2">
              <Link to="/" className={buttonVariants({ variant: 'outline' })}>
                Create a game
              </Link>
            </div>
          </Alert>
        ) : null}

        {state.status === 'ready' && decks.length === 0 ? (
          <EmptyState
            icon={LibraryIcon}
            title="You have no decks yet"
            action={
              <>
                {importAction}
                {newAction}
              </>
            }
          >
            Import a list from Arena, MTG Online, Moxfield or Archidekt, or start an empty deck. Each deck
            shows whether the game can play every card in it.
          </EmptyState>
        ) : null}

        {decks.length > 0 ? (
          // The gutter is kept, so the rows do not change width when a deleted deck ends the scrolling.
          <div className="min-h-0 flex-1 sm:overflow-y-auto sm:[scrollbar-gutter:stable]">
            <ul
              aria-label="Your decks"
              aria-busy={state.status === 'ready' && state.refreshing}
              className="grid gap-3"
            >
              {decks.map((deck) => (
                <DeckRow
                  key={deck.id}
                  deck={deck}
                  api={api}
                  now={now}
                  onDelete={(target) => open({ kind: 'delete', deck: target })}
                />
              ))}
            </ul>
          </div>
        ) : null}
      </section>

      {overlay?.kind === 'new' ? (
        <NewDeckDialog key={overlay.key} api={api} onSaved={saved} onClosed={() => closed(overlay.key)} />
      ) : null}
      {overlay?.kind === 'import' ? (
        <ImportDialog key={overlay.key} api={api} onSaved={saved} onClosed={() => closed(overlay.key)} />
      ) : null}
      {overlay?.kind === 'delete' ? (
        <DeleteDeckDialog
          key={overlay.key}
          api={api}
          deck={overlay.deck}
          onDeleted={deleted}
          onClosed={() => closed(overlay.key)}
        />
      ) : null}
    </PageShell>
  )
}
