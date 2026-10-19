import { Link, useRouter } from '@tanstack/react-router'
import { LoaderCircleIcon, ServerOffIcon } from 'lucide-react'
import { useEffect } from 'react'
import { EmptyState } from '@/components/EmptyState'
import { PageShell } from '@/components/PageShell'
import { buttonVariants } from '@/components/ui/button'
import { usePageTitle } from '@/hooks/usePageTitle'
import { AuthForm } from './AuthForm'
import { useAuth } from './context'
import type { AuthMode } from './errors'
import { sanitizeNext } from './next'

interface AuthScreenProps {
  mode: AuthMode
  /** Where to go once signed in; already checked by `sanitizeNext`. Home when absent. */
  next: string | undefined
}

/** `/login` and `/register`: the form, or a note that this server has no accounts, or the way on when already signed in. */
export function AuthScreen({ mode, next }: AuthScreenProps) {
  usePageTitle(mode === 'login' ? 'Sign in' : 'Create an account')
  const status = useAuth((state) => state.status)
  const router = useRouter()
  // Checked again here: a router can hand over the raw address where a validator left a key out, and this is the one
  // place that navigates to it.
  const safe = sanitizeNext(next) ?? undefined
  const destination = safe ?? '/'

  // Signing in, registering (which signs in) and arriving already signed in all end here.
  useEffect(() => {
    if (status === 'signed-in') void router.navigate({ href: destination, replace: true })
  }, [status, destination, router])

  return (
    <PageShell width="narrow" className="sm:pt-12">
      {status === 'unavailable' ? (
        <EmptyState
          as="h1"
          icon={ServerOffIcon}
          title="This server has no accounts"
          action={
            <Link to="/new" className={buttonVariants({ size: 'lg' })}>
              Create game
            </Link>
          }
        >
          Games here are played as a guest: one person creates a game and sends every player the link for
          their seat. There is nothing to sign in to.
        </EmptyState>
      ) : null}
      {status === 'signed-in' ? (
        <p role="status" className="surface flex items-center gap-3 p-4 text-sm text-muted-foreground">
          <LoaderCircleIcon aria-hidden="true" className="size-4 shrink-0 motion-safe:animate-spin" />
          You are signed in. Taking you on…
        </p>
      ) : null}
      {status === 'loading' || status === 'signed-out' ? <AuthForm mode={mode} next={safe} /> : null}
    </PageShell>
  )
}
