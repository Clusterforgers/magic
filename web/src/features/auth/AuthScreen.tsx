import { Link, useRouter } from '@tanstack/react-router'
import { useEffect } from 'react'
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
    <PageShell width="narrow">
      {status === 'unavailable' ? (
        <section aria-labelledby="no-accounts-title" className="grid justify-items-start gap-3">
          <h1 id="no-accounts-title" className="text-2xl font-semibold tracking-tight">
            This server has no accounts
          </h1>
          <p className="text-base text-muted-foreground">
            Games here are played as a guest: one person creates a game and sends every player the link for
            their seat. There is nothing to sign in to.
          </p>
          <Link to="/new" className={buttonVariants({ size: 'lg' })}>
            Create a game
          </Link>
        </section>
      ) : null}
      {status === 'signed-in' ? (
        <p role="status" className="text-sm text-muted-foreground">
          You are signed in. Taking you on…
        </p>
      ) : null}
      {status === 'loading' || status === 'signed-out' ? <AuthForm mode={mode} next={safe} /> : null}
    </PageShell>
  )
}
