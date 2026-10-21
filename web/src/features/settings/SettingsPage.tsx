import { Link, Navigate } from '@tanstack/react-router'
import { ServerOffIcon } from 'lucide-react'
import { useState } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { EmptyState } from '@/components/EmptyState'
import { Loading } from '@/components/Loading'
import { PageShell } from '@/components/PageShell'
import { buttonVariants } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { useAuth } from '@/features/auth/context'
import { usePageTitle } from '@/hooks/usePageTitle'
import { cn } from '@/lib/utils'
import { ChangePasswordForm } from './ChangePasswordForm'
import { DeleteAccountSection } from './DeleteAccountSection'

/**
 * `/settings`: change the password and delete the account. Without a session it sends the visitor to sign in and back
 * here; a server without accounts says so. A deleted account leaves the page for the home page by itself.
 */
export function SettingsPage() {
  usePageTitle('Account settings')
  const { status, user } = useAuth(useShallow((state) => ({ status: state.status, user: state.user })))
  const [deleting, setDeleting] = useState(false)

  if (status === 'loading') {
    return (
      <PageShell width="narrow" className="sm:pt-12">
        <Loading label="Checking your sign-in…">
          <Skeleton className="h-8 w-40" />
        </Loading>
      </PageShell>
    )
  }
  if (status === 'signed-out') {
    // The account just deleted is signed out too; that page is already on its way home.
    return (
      <PageShell width="narrow" className="sm:pt-12">
        {deleting ? null : <Navigate to="/login" search={{ next: '/settings' }} replace />}
        <p role="status" className="text-sm text-muted-foreground">
          {deleting
            ? 'Taking you home…'
            : 'You need to sign in to change your account. Taking you to the sign-in page…'}
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
          There is no account to change here. Games are played as a guest with a seat link.
        </EmptyState>
      </PageShell>
    )
  }

  return (
    <PageShell width="narrow" className="grid gap-6 sm:pt-12">
      <header className="grid gap-1">
        <h1 className="text-2xl font-semibold tracking-tight">Account settings</h1>
        <p className="text-base text-muted-foreground">
          Signed in as <span className="font-medium break-all text-foreground">{user?.username}</span>.
        </p>
      </header>
      <ChangePasswordForm />
      <DeleteAccountSection onDeleting={setDeleting} />
    </PageShell>
  )
}
