import { Link, Navigate } from '@tanstack/react-router'
import { ServerOffIcon, ShieldAlertIcon } from 'lucide-react'
import { useEffect } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { EmptyState } from '@/components/EmptyState'
import { Loading } from '@/components/Loading'
import { PageShell } from '@/components/PageShell'
import { buttonVariants } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { useAuth, useAuthStore } from '@/features/auth/context'
import { usePageTitle } from '@/hooks/usePageTitle'
import { cn } from '@/lib/utils'
import { AccountList } from './AccountList'
import { NewAccountForm } from './NewAccountForm'

/**
 * `/accounts`: the admin makes accounts for friends and sees who has one. Anyone else is told that only the admin
 * can; that is a courtesy, since the server judges every request. Without a session it sends the visitor to sign in
 * and back here.
 */
export function AccountsPage() {
  usePageTitle('Accounts')
  const { status, user } = useAuth(useShallow((state) => ({ status: state.status, user: state.user })))
  const store = useAuthStore()
  const isAdmin = status === 'signed-in' && user?.is_admin === true

  useEffect(() => {
    if (isAdmin) void store.getState().loadAccounts()
  }, [isAdmin, store])

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
        <Navigate to="/login" search={{ next: '/accounts' }} replace />
        <p role="status" className="text-sm text-muted-foreground">
          You need to sign in first. Taking you to the sign-in page…
        </p>
      </PageShell>
    )
  }
  if (status === 'unavailable') {
    return (
      <PageShell width="narrow" className="sm:pt-12">
        <EmptyState
          as="h1"
          icon={ServerOffIcon}
          title="This server has no accounts"
          action={
            <Link to="/new" className={cn(buttonVariants({ size: 'lg' }))}>
              Create game
            </Link>
          }
        >
          There are no accounts to make here. Games are played as a guest with a seat link.
        </EmptyState>
      </PageShell>
    )
  }
  if (!isAdmin) {
    return (
      <PageShell width="narrow" className="sm:pt-12">
        <EmptyState
          as="h1"
          icon={ShieldAlertIcon}
          title="Only the admin can make accounts."
          action={
            <Link to="/" className={cn(buttonVariants({ size: 'lg' }))}>
              Go home
            </Link>
          }
        >
          Your account is not the admin. Ask the admin to add the friends you want to play with.
        </EmptyState>
      </PageShell>
    )
  }

  return (
    <PageShell width="page" className="grid gap-6 sm:pt-12">
      <header className="grid gap-1">
        <h1 className="text-2xl font-semibold tracking-tight">Accounts</h1>
        <p className="text-base text-muted-foreground">
          You are the admin: only you can make accounts. Friends cannot register themselves.
        </p>
      </header>
      <NewAccountForm />
      <AccountList />
    </PageShell>
  )
}
