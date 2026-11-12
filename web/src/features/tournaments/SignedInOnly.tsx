import { Navigate } from '@tanstack/react-router'
import { ServerOffIcon } from 'lucide-react'
import type { ReactNode } from 'react'
import { EmptyState } from '@/components/EmptyState'
import { Loading } from '@/components/Loading'
import { Skeleton } from '@/components/ui/skeleton'
import { useAuth } from '@/features/auth/context'

/** Shows `children` to a signed-in player; sends anybody else to sign in and back to `next`. */
export function SignedInOnly({ next, children }: { next: string; children: ReactNode }) {
  const status = useAuth((state) => state.status)
  if (status === 'loading') {
    return (
      <Loading label="Checking your sign-in…">
        <Skeleton className="h-8 w-40" />
      </Loading>
    )
  }
  if (status === 'signed-out') {
    return (
      <>
        <Navigate to="/login" search={{ next }} replace />
        <p role="status" className="text-sm text-muted-foreground">
          You need to sign in for tournaments. Taking you to the sign-in page…
        </p>
      </>
    )
  }
  if (status === 'unavailable') {
    return (
      <EmptyState icon={ServerOffIcon} title="No tournaments here">
        This server has no accounts, so it runs no tournaments.
      </EmptyState>
    )
  }
  return <>{children}</>
}
