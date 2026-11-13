import { Link, useNavigate } from '@tanstack/react-router'
import {
  FolderPlusIcon,
  ListChecksIcon,
  CircleAlertIcon,
  LibraryIcon,
  PlusIcon,
  TriangleAlertIcon,
  UploadIcon,
} from 'lucide-react'
import { useCallback, useRef, useState } from 'react'
import { EmptyState } from '@/components/EmptyState'
import { Loading } from '@/components/Loading'
import { PageShell } from '@/components/PageShell'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button, buttonVariants } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { deckGroupsApi as sharedGroups } from '@/net/deckGroups'
import type { DeckGroupsApi } from '@/net/deckGroups'
import type { DecksApi } from '@/net/decks'
import type { DeckGroupView, DeckListItem, DeckView } from '@/protocol/generated'
import { DeckRow } from './DeckRow'
import { DeleteDeckDialog } from './DeleteDeckDialog'
import { GroupedDecks } from './groups/GroupedDecks'
import { GroupNameDialog } from './groups/GroupNameDialog'
import { PRESET_GROUPS } from './groups/grouping'
import { useDeckGroups } from './groups/useDeckGroups'
import { ImportDialog } from './ImportDialog'
import { NewDeckDialog } from './NewDeckDialog'
import { useDeckList } from './useDeckList'

type Overlay =
  | { key: number; kind: 'new' }
  | { key: number; kind: 'import' }
  | { key: number; kind: 'delete'; deck: DeckListItem }
  | { key: number; kind: 'group'; group?: DeckGroupView }

const NO_DECKS: DeckListItem[] = []

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
  /** The deck groups; the shared client by default. */
  groupsApi?: DeckGroupsApi
}

/**
 * The signed-in user's decks: the list with each deck's check state, and New deck, Import and Delete. From the `sm`
 * breakpoint up the page is as tall as the window under the header and the list scrolls inside its own region.
 */
export function DeckLibrary({ api, clock, groupsApi = sharedGroups }: DeckLibraryProps) {
  const { state, reload } = useDeckList(api)
  const groups = useDeckGroups(groupsApi, state.status === 'ready')
  const [overlay, setOverlay] = useState<Overlay | null>(null)
  const [announcement, setAnnouncement] = useState('')
  const opened = useRef(0)
  const titleRef = useRef<HTMLHeadingElement>(null)
  const headingNext = useRef(false)
  const navigate = useNavigate()

  const open = useCallback(
    (
      next:
        | { kind: 'new' }
        | { kind: 'import' }
        | { kind: 'delete'; deck: DeckListItem }
        | { kind: 'group'; group?: DeckGroupView },
    ): void => {
      opened.current += 1
      setOverlay({ ...next, key: opened.current })
    },
    [],
  )
  // Stable, so the memoised rows draw again only when their deck changes.
  const askDelete = useCallback((target: DeckListItem) => open({ kind: 'delete', deck: target }), [open])
  const { file } = groups
  const fileDeck = useCallback((deck: DeckListItem, group: string | null) => file(deck.id, group), [file])
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

  const decks = state.status === 'ready' ? state.decks : NO_DECKS
  // The ages are counted to the moment the list arrived, so a row has no reason to draw again until the next load.
  const [aged, setAged] = useState(() => ({ decks, now: clock() }))
  if (aged.decks !== decks) setAged({ decks, now: clock() })
  const now = aged.decks === decks ? aged.now : clock()
  const importAction = (
    <Button
      type="button"
      aria-haspopup="dialog"
      data-tour="import-deck"
      onClick={() => open({ kind: 'import' })}
    >
      <UploadIcon aria-hidden="true" />
      Import deck
    </Button>
  )
  const newAction = (
    <Button
      type="button"
      variant="outline"
      aria-haspopup="dialog"
      data-tour="new-deck"
      onClick={() => open({ kind: 'new' })}
    >
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
              <Link to="/review" search={{ mine: true }} className={buttonVariants({ variant: 'ghost' })}>
                <ListChecksIcon aria-hidden="true" />
                Review cards
              </Link>
              {groups.ready ? (
                <Button
                  type="button"
                  variant="ghost"
                  aria-haspopup="dialog"
                  onClick={() => open({ kind: 'group' })}
                >
                  <FolderPlusIcon aria-hidden="true" />
                  New group
                </Button>
              ) : null}
              {newAction}
              {importAction}
            </div>
          )}
        </div>
        <p role="status" className="sr-only">
          {announcement}
        </p>
        {groups.problem === null ? null : (
          <Alert variant="destructive">
            <CircleAlertIcon aria-hidden="true" />
            <AlertTitle>The groups were not saved</AlertTitle>
            <AlertDescription>{groups.problem}</AlertDescription>
          </Alert>
        )}
        {groups.ready && groups.groups.length === 0 && decks.length > 1 ? (
          <div
            data-slot="group-presets"
            className="flex flex-wrap items-center gap-x-3 gap-y-2 text-sm text-muted-foreground"
          >
            <span>Sort your decks into groups and fold away the ones you are not playing.</span>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => {
                groups.create(PRESET_GROUPS)
                setAnnouncement(`Added the groups ${PRESET_GROUPS.join(', ')}.`)
              }}
            >
              Add {PRESET_GROUPS.slice(0, -1).join(', ')} and {PRESET_GROUPS.at(-1)}
            </Button>
          </div>
        ) : null}

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
            {groups.ready && groups.groups.length > 0 ? (
              <GroupedDecks
                decks={decks}
                groups={groups}
                onRename={(group) => open({ kind: 'group', group })}
                onAnnounce={setAnnouncement}
                renderRow={(deck, groupId) => (
                  <DeckRow
                    key={deck.id}
                    deck={deck}
                    api={api}
                    now={now}
                    onDelete={askDelete}
                    groups={groups.groups}
                    groupId={groupId}
                    onFile={fileDeck}
                  />
                )}
              />
            ) : (
              <ul
                aria-label="Your decks"
                aria-busy={state.status === 'ready' && state.refreshing}
                className="grid gap-3"
              >
                {decks.map((deck) => (
                  <DeckRow key={deck.id} deck={deck} api={api} now={now} onDelete={askDelete} />
                ))}
              </ul>
            )}
          </div>
        ) : null}
      </section>

      {overlay?.kind === 'group' ? (
        <GroupNameDialog
          key={overlay.key}
          initial={overlay.group?.name}
          onSubmit={(name) => {
            if (overlay.group === undefined) {
              groups.create([name])
              setAnnouncement(`Added the group “${name}”.`)
            } else {
              groups.rename(overlay.group.id, name)
              setAnnouncement(`Renamed the group to “${name}”.`)
            }
          }}
          onClosed={() => closed(overlay.key)}
        />
      ) : null}

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
