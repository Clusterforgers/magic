import { Navigate } from '@tanstack/react-router'
import { Loading } from '@/components/Loading'
import { PageShell } from '@/components/PageShell'
import { Skeleton } from '@/components/ui/skeleton'
import { useAuth } from '@/features/auth/context'
import type { ByHandApi } from '@/features/decks/byHand'
import { usePageTitle } from '@/hooks/usePageTitle'
import { readPath } from '@/lib/toRead'
import type { ReadTarget } from '@/lib/toRead'
import type { BulkReadsApi } from '@/net/bulkReads'
import { readsApi } from '@/net/reads'
import type { ReadsApi } from '@/net/reads'
import { queueWords } from './model'
import { ReadView } from './ReadView'
import type { CheckApis } from './useCheckCards'

interface ReadPageProps {
  /** The cards read, from the address: a deck, a precon or the account's decks; null for every card waiting for a first read. */
  target: ReadTarget | null
  /** The read endpoints; the shared client unless a test brings its own. */
  api?: ReadsApi
  /** The deck endpoints that mark a card found wrong "play by hand"; the shared client unless a test brings its own. */
  decks?: ByHandApi
  /** The endpoints the cards to check come from; the shared clients unless a test brings its own. */
  checks?: CheckApis
  /** The bulk read endpoints; the shared client unless a test brings its own. */
  bulk?: BulkReadsApi
}

function Notice({ children, action }: { children: string; action?: React.ReactNode }) {
  return (
    <PageShell width="wide">
      <section aria-labelledby="read-title" className="grid justify-items-start gap-3">
        <h1 id="read-title" className="text-2xl font-semibold tracking-tight">
          Review cards
        </h1>
        <p className="text-base text-muted-foreground">{children}</p>
        {action}
      </section>
    </PageShell>
  )
}

/**
 * `/review` (every card), `/review?mine=true` (the account's decks), `/review?deck=<id>` and `/review?precon=<code>`: the review screen, where a signed-in player
 * reads the drafts of cards, says whether each reads right, and checks the cards played by hand. Someone who is not signed in is sent to the sign-in form and back.
 */
export function ReadPage({ target, api = readsApi, decks, checks, bulk }: ReadPageProps) {
  usePageTitle(target !== null && 'mine' in target ? queueWords(target, '').heading : 'Review cards')
  const status = useAuth((state) => state.status)

  const path = target === null ? '/review' : readPath(target)
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
          You need to sign in to review cards. Taking you to the sign-in page…
        </p>
      </PageShell>
    )
  }
  if (status === 'unavailable') {
    return <Notice>This server has no accounts, so nobody can review its cards here.</Notice>
  }
  return (
    <PageShell width="wide">
      <ReadView
        key={path}
        api={api}
        target={target}
        {...(decks === undefined ? {} : { decks })}
        {...(checks === undefined ? {} : { checks })}
        {...(bulk === undefined ? {} : { bulk })}
      />
    </PageShell>
  )
}
