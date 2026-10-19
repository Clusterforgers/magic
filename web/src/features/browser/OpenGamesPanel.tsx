import { Link } from '@tanstack/react-router'
import { CircleAlertIcon, PlusIcon, RefreshCwIcon, SwordsIcon } from 'lucide-react'
import { useMemo, useRef, useState } from 'react'
import { EmptyState } from '@/components/EmptyState'
import { Loading } from '@/components/Loading'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button, buttonVariants } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { describeLobbyError } from '@/net/lobbyErrors'
import { FocusKeeper } from './FocusKeeper'
import { GameFilters } from './GameFilters'
import { filterGames, FILTERS_ABOVE_ROWS, NO_FILTERS, openGamesText } from './format'
import type { Filters } from './format'
import type { OnJoin } from './joinTarget'
import { OpenGameRows } from './OpenGameRows'
import { CLOCK_MS, useClock } from './useClock'
import type { OpenGames } from './useOpenGames'

interface Props {
  open: OpenGames
  onJoin: OnJoin | undefined
  /** Sends a visitor with no game to the code field. */
  onFocusCode(): void
}

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

/**
 * The open games: a heading with the count and Refresh, filters above a long list, an alert with Retry when the last
 * request failed (the rows stay, dimmed), and the rows in a region that scrolls inside itself.
 */
export function OpenGamesPanel({ open, onJoin, onFocusCode }: Props) {
  const { listing, error, refreshing, refresh } = open
  const [chosen, setChosen] = useState<Filters>(NO_FILTERS)
  const now = useClock(CLOCK_MS)
  const section = useRef<HTMLElement>(null)
  const refreshButton = useRef<HTMLButtonElement>(null)

  const games = useMemo(() => listing?.games ?? [], [listing])
  const filterable = games.length > FILTERS_ABOVE_ROWS
  // A choice only counts while its controls are in the page, so a list that shrinks cannot stay filtered unseen.
  const filters = filterable ? chosen : NO_FILTERS
  const rows = useMemo(() => filterGames(games, filters), [games, filters])
  const stale = error !== null && listing !== null

  let body = null
  if (listing === null) {
    body =
      error === null ? (
        <Loading label="Loading the open games…">
          <RowsSkeleton />
        </Loading>
      ) : null
  } else if (games.length === 0) {
    body = (
      <EmptyState
        icon={SwordsIcon}
        title="No open games"
        action={
          <>
            <Link to="/new" className={buttonVariants()}>
              <PlusIcon aria-hidden="true" />
              Create game
            </Link>
            <Button type="button" variant="outline" onClick={onFocusCode}>
              Join with a code
            </Button>
          </>
        }
      >
        Create one, or join with a code.
      </EmptyState>
    )
  } else if (rows.length === 0) {
    body = (
      <EmptyState
        title="No games match these filters"
        action={
          <Button type="button" variant="outline" onClick={() => setChosen(NO_FILTERS)}>
            Clear filters
          </Button>
        }
      >
        Change a filter to see the other open games.
      </EmptyState>
    )
  } else {
    body = (
      <div
        role="region"
        aria-labelledby="open-games-title"
        tabIndex={0}
        data-stale={stale || undefined}
        className="surface min-h-0 overflow-y-auto overscroll-contain"
      >
        <OpenGameRows rows={rows} fetchedAt={listing.fetchedAt} now={now} stale={stale} onJoin={onJoin} />
      </div>
    )
  }

  return (
    // Not a labelled region: the scrolling region inside it is the landmark, and one name should not mark two.
    <section ref={section} className="flex min-h-0 flex-col gap-3">
      <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
        <div className="flex flex-wrap items-baseline gap-x-3">
          <h2 id="open-games-title" className="text-lg font-semibold">
            Open games
          </h2>
          {/* A status that is always in the page, so a change of its words is announced. */}
          <p role="status" className="text-sm text-muted-foreground">
            {listing === null ? '' : openGamesText(rows.length, games.length)}
          </p>
        </div>
        <Button
          ref={refreshButton}
          type="button"
          variant="outline"
          size="sm"
          loading={refreshing}
          onClick={refresh}
        >
          {refreshing ? null : <RefreshCwIcon aria-hidden="true" />}
          Refresh
        </Button>
      </div>

      {error === null ? null : (
        <Alert variant="destructive">
          <CircleAlertIcon aria-hidden="true" />
          <AlertTitle>
            {stale ? 'Could not refresh the open games' : 'Could not load the open games'}
          </AlertTitle>
          <AlertDescription>
            {describeLobbyError(error)}
            {stale ? ' The games below may be out of date.' : ''}
          </AlertDescription>
          <div className="mt-2">
            <Button type="button" loading={refreshing} onClick={refresh}>
              Retry
            </Button>
          </div>
        </Alert>
      )}

      {listing !== null && filterable ? (
        <GameFilters games={games} filters={chosen} onChange={setChosen} />
      ) : null}

      <FocusKeeper rows={rows} within={section} fallback={refreshButton}>
        {body}
      </FocusKeeper>
    </section>
  )
}
