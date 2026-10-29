import { CircleAlertIcon, LayersIcon } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useFitWindow } from '@/components/appChrome'
import { EmptyState } from '@/components/EmptyState'
import { Loading } from '@/components/Loading'
import { PageShell } from '@/components/PageShell'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { GuestJoinDialog } from '@/features/browser/GuestJoinDialog'
import { usePageTitle } from '@/hooks/usePageTitle'
import { api as sharedApi } from '@/net/api'
import type { Api } from '@/net/api'
import { cardsApi } from '@/net/cards'
import type { CardsApi } from '@/net/cards'
import { preconsApi } from '@/net/precons'
import type { PreconsApi } from '@/net/precons'
import type { PreconListItem } from '@/protocol/generated'
import { describePreconError, isUnavailable } from './errors'
import { countLine, filterPrecons, NO_FILTERS, setOptions } from './format'
import type { Filters } from './format'
import { PreconDialog } from './PreconDialog'
import { PreconFilters } from './PreconFilters'
import { PreconRows } from './PreconRows'
import { usePlayPrecon } from './usePlayPrecon'
import { usePreconDetail } from './usePreconDetail'
import { usePrecons } from './usePrecons'

/** What `PreconsPage` takes. */
export interface PreconsPageProps {
  /** The precon calls; the shared client by default. */
  api?: Pick<PreconsApi, 'list' | 'get'>
  /** The card endpoint the clauses of a precon's cards are quoted from. */
  cards?: Pick<CardsApi, 'get'>
  /** The call that makes a game; the shared client by default. */
  games?: Pick<Api, 'createGame'>
}

/** A precon whose details are open. */
interface Details {
  precon: PreconListItem
  opener: HTMLElement | null
  /** False from the moment the dialog is asked to close until it has finished closing. */
  open: boolean
}

// The shape of the list while it loads: a few rows like the ones that will come.
function RowsSkeleton() {
  return (
    <div className="surface grid gap-3 p-4">
      {[0, 1, 2, 3, 4].map((row) => (
        <Skeleton key={row} className="h-10" />
      ))}
    </div>
  )
}

/**
 * `/precons`: every precon deck of the server with the game's check of it (the whole deck runs, some cards are played
 * by hand, or the game cannot take it), filters by name, set, commander and check, the details of a deck and a Play
 * button that makes a game with it and opens the lobby. The page fits the window under the header; only the list
 * scrolls. A signed-in visitor plays in one click; a guest gives a name first.
 */
export function PreconsPage({ api = preconsApi, games = sharedApi, cards = cardsApi }: PreconsPageProps) {
  usePageTitle('Precons')
  useFitWindow()
  const list = usePrecons(api)
  const play = usePlayPrecon(games)
  const [filters, setFilters] = useState<Filters>(NO_FILTERS)
  const [details, setDetails] = useState<Details | null>(null)
  const detail = usePreconDetail(api, details?.precon.code ?? null)

  const precons = useMemo(() => (list.state.status === 'ready' ? list.state.precons : []), [list.state])
  const sets = useMemo(() => setOptions(precons), [precons])
  const rows = useMemo(() => filterPrecons(precons, filters), [precons, filters])
  const { asking, failure } = play

  const openDetails = (precon: PreconListItem, opener: HTMLElement | null): void =>
    setDetails({ precon, opener, open: true })

  let body = null
  if (list.state.status === 'loading') {
    body = (
      <Loading label="Loading the precons…">
        <RowsSkeleton />
      </Loading>
    )
  } else if (list.state.status === 'failed') {
    body = isUnavailable(list.state.error) ? (
      <EmptyState icon={LayersIcon} title="No card data on this server">
        This server has no card database, so it has no precons. Ask whoever runs it to load the cards.
      </EmptyState>
    ) : null
  } else if (precons.length === 0) {
    body = (
      <EmptyState icon={LayersIcon} title="No precons loaded">
        This server has no precon decks yet. Ask whoever runs it to load them.
      </EmptyState>
    )
  } else if (rows.length === 0) {
    body = (
      <EmptyState
        title="No precons match"
        action={
          <Button type="button" variant="outline" onClick={() => setFilters(NO_FILTERS)}>
            Clear filters
          </Button>
        }
      >
        Change the search or a filter to see the other precons.
      </EmptyState>
    )
  } else {
    body = (
      <div
        role="region"
        aria-labelledby="precons-title"
        tabIndex={0}
        className="surface min-h-0 overflow-y-auto overscroll-contain"
      >
        <PreconRows
          rows={rows}
          onDetails={openDetails}
          onPlay={(precon, opener) => play.start(precon, opener)}
          playing={play.playing}
          busy={play.busy}
        />
      </div>
    )
  }

  return (
    <PageShell width="wide" className="flex min-h-0 flex-1 flex-col gap-4 sm:py-6">
      <header className="grid gap-1">
        <div className="flex flex-wrap items-baseline gap-x-3">
          <h1 id="precons-title" className="text-2xl font-semibold tracking-tight">
            Precons
          </h1>
          {/* A status that is always in the page, so a change of its words is announced. */}
          <p role="status" className="text-sm text-muted-foreground">
            {list.state.status === 'ready' ? countLine(rows.length, precons.length) : ''}
          </p>
        </div>
        <p className="text-base text-muted-foreground">
          Ready-made decks. Pick one and play: nothing to import, and the check says which cards the game
          plays by hand.
        </p>
      </header>

      {list.state.status === 'failed' && !isUnavailable(list.state.error) ? (
        <Alert variant="destructive">
          <CircleAlertIcon aria-hidden="true" />
          <AlertTitle>Could not load the precons</AlertTitle>
          <AlertDescription>{describePreconError(list.state.error)}</AlertDescription>
          <div className="col-start-2 mt-2">
            <Button type="button" onClick={list.reload}>
              Retry
            </Button>
          </div>
        </Alert>
      ) : null}

      {failure === null ? null : (
        <Alert variant="destructive" data-slot="play-failure">
          <CircleAlertIcon aria-hidden="true" />
          <AlertTitle>{`Could not start a game with ${failure.precon.name}`}</AlertTitle>
          <AlertDescription>{failure.text}</AlertDescription>
          <div className="col-start-2 mt-2 flex flex-wrap gap-2">
            {failure.blocked ? (
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={(event) => {
                  openDetails(failure.precon, event.currentTarget)
                  play.dismiss()
                }}
              >
                Show why
              </Button>
            ) : null}
            <Button type="button" variant="outline" size="sm" onClick={play.dismiss}>
              Dismiss
            </Button>
          </div>
        </Alert>
      )}

      {list.state.status === 'ready' && precons.length > 0 ? (
        <PreconFilters filters={filters} sets={sets} onChange={setFilters} />
      ) : null}

      <div className="flex min-h-0 flex-col">{body}</div>

      {details === null ? null : (
        <PreconDialog
          key={details.precon.code}
          precon={details.precon}
          open={details.open}
          detail={detail.state}
          cards={cards}
          onRetry={detail.reload}
          onCancel={() => setDetails((was) => (was === null ? null : { ...was, open: false }))}
          onClosed={() => setDetails(null)}
          returnFocus={() => (details.opener?.isConnected === true ? details.opener : null)}
          onPlay={() => {
            setDetails((was) => (was === null ? null : { ...was, open: false }))
            play.start(details.precon, details.opener)
          }}
          playing={play.playing === details.precon.code}
          busy={play.busy}
        />
      )}

      {asking === null ? null : (
        <GuestJoinDialog
          title={`Play ${asking.precon.name}`}
          submitLabel="Play"
          open={asking.open}
          pending={play.busy}
          failure={play.dialogFailure}
          returnFocus={() => (asking.opener?.isConnected === true ? asking.opener : null)}
          onSubmit={play.submit}
          onCancel={play.cancel}
          onClosed={play.closed}
        />
      )}
    </PageShell>
  )
}
