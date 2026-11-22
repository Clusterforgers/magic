import { Navigate } from '@tanstack/react-router'
import { ChartNoAxesColumnIcon, CircleAlertIcon, ServerOffIcon } from 'lucide-react'
import { useEffect, useState } from 'react'
import { EmptyState } from '@/components/EmptyState'
import { Loading } from '@/components/Loading'
import { PageShell } from '@/components/PageShell'
import { Alert, AlertAction, AlertDescription } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { useAuth } from '@/features/auth/context'
import { usePageTitle } from '@/hooks/usePageTitle'
import { BY_HAND } from '@/lib/byHand'
import { cardSupportApi } from '@/net/cardSupport'
import type { CardSupportApi } from '@/net/cardSupport'
import { BucketList } from './BucketList'
import { FeedList } from './FeedList'
import { Headline } from './Headline'
import { MissingList } from './MissingList'
import { checkWords } from './model'
import { useCardSupport } from './useCardSupport'

/** The page's title and the line under it. */
function Header({ check }: { check: string | null }) {
  return (
    <header className="grid gap-1">
      <h1 className="font-display text-2xl font-semibold tracking-tight">Card support</h1>
      <p className="text-base text-muted-foreground">
        {`How many Commander-legal cards the game plays automatically, and what is still ${BY_HAND}. New cards and precons are looked for every day.`}
      </p>
      {check === null ? null : (
        <p className="text-sm text-muted-foreground" data-slot="check">
          {check}
        </p>
      )}
    </header>
  )
}

/** The page for a signed-in account: the numbers, refreshed while it is open. */
function CardSupportView({ api }: { api: CardSupportApi }) {
  const { overview, missing, error, fetchedAt, updated, retry } = useCardSupport(api)
  // Ages read "3 hours ago"; a minute's tick keeps them true while the page stays open.
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 60_000)
    return () => clearInterval(timer)
  }, [])
  const clock = Math.max(now, fetchedAt)

  const failure =
    error === null ? null : (
      <Alert variant="destructive">
        <CircleAlertIcon aria-hidden="true" />
        <AlertDescription className="text-foreground">
          {overview === null ? `Could not load card support. ${error}` : `Could not refresh. ${error}`}
        </AlertDescription>
        <AlertAction>
          <Button type="button" variant="ghost" size="xs" onClick={retry}>
            Try again
          </Button>
        </AlertAction>
      </Alert>
    )

  if (overview === null) {
    return (
      <>
        <Header check={null} />
        {failure ?? (
          <Loading label="Loading card support…">
            <div className="grid gap-4">
              <Skeleton className="h-40 w-full" />
              <Skeleton className="h-64 w-full" />
            </div>
          </Loading>
        )}
      </>
    )
  }

  const { snapshot } = overview
  return (
    <>
      <Header check={checkWords(overview.last_check, clock)} />
      {failure}
      <p role="status" className="sr-only">
        {updated === null ? '' : 'Card support was updated with new numbers.'}
      </p>
      {snapshot === null ? (
        <EmptyState icon={ChartNoAxesColumnIcon} title="No numbers yet">
          The first count runs with the next daily check or release.
        </EmptyState>
      ) : (
        <>
          <Headline snapshot={snapshot} history={overview.history} />
          <div className="grid gap-6 lg:grid-cols-[minmax(0,20rem)_minmax(0,1fr)]">
            <div className="grid content-start gap-6">
              <BucketList buckets={snapshot.buckets} />
              <FeedList feed={overview.feed} now={clock} />
            </div>
            {missing === null ? (
              <Loading label="Loading the missing cards…">
                <Skeleton className="h-64 w-full" />
              </Loading>
            ) : (
              <MissingList cards={missing.cards} />
            )}
          </div>
        </>
      )}
    </>
  )
}

/**
 * `/support`: the share of Commander-legal cards that play automatically, by state and popularity, its trend, the feed
 * of new sets, precons and cards now automatic, and the searchable list of what is missing. It needs a session; a
 * server without accounts says so.
 */
export function CardSupportPage({ api = cardSupportApi }: { api?: CardSupportApi }) {
  usePageTitle('Card support')
  const status = useAuth((state) => state.status)
  return (
    <PageShell width="wide" className="grid gap-6 sm:pt-12">
      {status === 'loading' ? (
        <Loading label="Checking your sign-in…">
          <Skeleton className="h-8 w-40" />
        </Loading>
      ) : status === 'signed-out' ? (
        <>
          <Navigate to="/login" search={{ next: '/support' }} replace />
          <p role="status" className="text-sm text-muted-foreground">
            You need to sign in first. Taking you to the sign-in page…
          </p>
        </>
      ) : status === 'unavailable' ? (
        <EmptyState icon={ServerOffIcon} title="No card support here">
          This server has no accounts, so it shows no card numbers.
        </EmptyState>
      ) : (
        <CardSupportView api={api} />
      )}
    </PageShell>
  )
}
