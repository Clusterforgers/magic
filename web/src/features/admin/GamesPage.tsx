import { Link, Navigate } from '@tanstack/react-router'
import { ShieldAlertIcon } from 'lucide-react'
import { useShallow } from 'zustand/react/shallow'
import { EmptyState } from '@/components/EmptyState'
import { Loading } from '@/components/Loading'
import { PageShell } from '@/components/PageShell'
import { buttonVariants } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { useAuth } from '@/features/auth/context'
import { usePageTitle } from '@/hooks/usePageTitle'
import { cn } from '@/lib/utils'
import { adminGamesApi } from '@/net/adminGames'
import type { AdminGamesApi } from '@/net/adminGames'
import { LiveGames } from './LiveGames'

/**
 * `/admin/games`: an admin sees every lobby and running game and ends any of them. Anyone else is told that only an
 * admin can; the server judges every request. Without a session it sends the visitor to sign in and back here.
 */
export function GamesPage({ api = adminGamesApi }: { api?: AdminGamesApi }) {
  usePageTitle('Live games')
  const { status, user } = useAuth(useShallow((state) => ({ status: state.status, user: state.user })))
  const isAdmin = status === 'signed-in' && user?.is_admin === true

  if (status === 'loading') {
    return (
      <PageShell width="page" className="sm:pt-12">
        <Loading label="Checking your sign-in…">
          <Skeleton className="h-8 w-40" />
        </Loading>
      </PageShell>
    )
  }
  if (status === 'signed-out') {
    return (
      <PageShell width="page" className="sm:pt-12">
        <Navigate to="/login" search={{ next: '/admin/games' }} replace />
        <p role="status" className="text-sm text-muted-foreground">
          You need to sign in first. Taking you to the sign-in page…
        </p>
      </PageShell>
    )
  }
  if (!isAdmin) {
    return (
      <PageShell width="narrow" className="sm:pt-12">
        <EmptyState
          as="h1"
          icon={ShieldAlertIcon}
          title="Only an admin can see the live games."
          action={
            <Link to="/" className={cn(buttonVariants({ size: 'lg' }))}>
              Go home
            </Link>
          }
        >
          Your account is not an admin.
        </EmptyState>
      </PageShell>
    )
  }

  return (
    <PageShell width="page" className="grid gap-6 sm:pt-12">
      <header className="grid gap-1">
        <h1 className="text-2xl font-semibold tracking-tight">Live games</h1>
        <p className="text-base text-muted-foreground">
          Every lobby and running game on this server. Ending one stops it for good, with no winner; its
          players read that an admin ended it.
        </p>
      </header>
      <LiveGames api={api} />
    </PageShell>
  )
}
