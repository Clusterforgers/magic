import { Navigate } from '@tanstack/react-router'
import { Loading } from '@/components/Loading'
import { PageShell } from '@/components/PageShell'
import { Skeleton } from '@/components/ui/skeleton'
import { useAuth } from '@/features/auth/context'
import type { ByHandApi } from '@/features/decks/byHand'
import { usePageTitle } from '@/hooks/usePageTitle'
import { readPath } from '@/lib/toRead'
import type { ReadTarget } from '@/lib/toRead'
import { readsApi } from '@/net/reads'
import type { ReadsApi } from '@/net/reads'
import { ReadView } from './ReadView'

interface ReadPageProps {
  /** The deck whose cards are read, from the address; null for every card waiting for a first read. */
  target: ReadTarget | null
  /** The read endpoints; the shared client unless a test brings its own. */
  api?: ReadsApi
  /** The deck endpoints that mark a card found wrong "play by hand"; the shared client unless a test brings its own. */
  decks?: ByHandApi
}

function Notice({ children, action }: { children: string; action?: React.ReactNode }) {
  return (
    <PageShell width="wide">
      <section aria-labelledby="read-title" className="grid justify-items-start gap-3">
        <h1 id="read-title" className="text-2xl font-semibold tracking-tight">
          Read cards
        </h1>
        <p className="text-base text-muted-foreground">{children}</p>
        {action}
      </section>
    </PageShell>
  )
}

/**
 * `/read` (every card), `/read?deck=<id>` and `/read?precon=<code>`: the review screen, where a signed-in player reads
 * the drafts of cards and says whether each is correct. Someone who is not signed in is sent to the sign-in form and back.
 */
export function ReadPage({ target, api = readsApi, decks }: ReadPageProps) {
  usePageTitle('Read cards')
  const status = useAuth((state) => state.status)

  const path = target === null ? '/read' : readPath(target)
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
          You need to sign in to read cards. Taking you to the sign-in page…
        </p>
      </PageShell>
    )
  }
  if (status === 'unavailable') {
    return <Notice>This server has no accounts, so nobody can read its cards here.</Notice>
  }
  return (
    <PageShell width="wide">
      <ReadView key={path} api={api} target={target} {...(decks === undefined ? {} : { decks })} />
    </PageShell>
  )
}
