import { Navigate } from '@tanstack/react-router'
import { Loading } from '@/components/Loading'
import { PageShell } from '@/components/PageShell'
import { Skeleton } from '@/components/ui/skeleton'
import { useAuth } from '@/features/auth/context'
import { usePageTitle } from '@/hooks/usePageTitle'
import { bulkReadsApi } from '@/net/bulkReads'
import type { BulkReadsApi } from '@/net/bulkReads'
import { BulkReadView } from './BulkReadView'

/**
 * `/read/bulk`: a signed-in player approves, in one press, the drafts whose clauses differ from the card only in
 * wording. Someone who is not signed in is sent to the sign-in form and back.
 */
export function BulkReadPage({ api = bulkReadsApi }: { api?: BulkReadsApi }) {
  usePageTitle('Wording-only reads')
  const status = useAuth((state) => state.status)
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
        <Navigate to="/login" search={{ next: '/read/bulk' }} replace />
        <p role="status" className="text-sm text-muted-foreground">
          You need to sign in to read cards. Taking you to the sign-in page…
        </p>
      </PageShell>
    )
  }
  if (status === 'unavailable') {
    return (
      <PageShell width="wide">
        <p className="text-base text-muted-foreground">
          This server has no accounts, so nobody can read its cards here.
        </p>
      </PageShell>
    )
  }
  return (
    <PageShell width="wide">
      <BulkReadView api={api} />
    </PageShell>
  )
}
