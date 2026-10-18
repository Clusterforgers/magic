import { Link, useLocation, useNavigate } from '@tanstack/react-router'
import { useState } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { Alert, AlertAction, AlertDescription } from '@/components/ui/alert'
import { Button, buttonVariants } from '@/components/ui/button'
import { useAuth, useAuthStore } from './context'

// Pages that only mean something while signed in: signing out there goes home instead of to the sign-in form.
const SIGNED_IN_ONLY: ReadonlySet<string> = new Set(['/games'])

/**
 * The account part of the header. Nothing while the server is still being asked or when it has no accounts; "Sign
 * in" and "Register" for a guest; the username, "My games" and "Sign out" for an account.
 */
export function AccountNav() {
  const { status, user } = useAuth(useShallow((state) => ({ status: state.status, user: state.user })))
  const store = useAuthStore()
  const navigate = useNavigate()
  const pathname = useLocation({ select: (location) => location.pathname })
  const [signingOut, setSigningOut] = useState(false)

  if (status === 'signed-out') {
    return (
      <nav aria-label="Account" className="flex items-center gap-1">
        <Link to="/login" className={buttonVariants({ variant: 'ghost', size: 'sm' })}>
          Sign in
        </Link>
        <Link to="/register" className={buttonVariants({ variant: 'outline', size: 'sm' })}>
          Register
        </Link>
      </nav>
    )
  }
  if (status !== 'signed-in' || user === null) return null

  async function signOut(): Promise<void> {
    setSigningOut(true)
    // First, so the page that needs an account does not send the visitor to the sign-in form they just left.
    if (SIGNED_IN_ONLY.has(pathname)) await navigate({ to: '/' })
    await store.getState().logout()
    setSigningOut(false)
  }

  return (
    <nav aria-label="Account" className="flex min-w-0 items-center gap-1">
      <span
        className="min-w-0 max-w-28 truncate px-1 text-sm text-muted-foreground sm:max-w-56"
        title={user.username}
      >
        <span className="sr-only">Signed in as </span>
        {user.username}
      </span>
      <Link to="/games" className={buttonVariants({ variant: 'ghost', size: 'sm' })}>
        My games
      </Link>
      <Button type="button" variant="outline" size="sm" disabled={signingOut} onClick={() => void signOut()}>
        Sign out
      </Button>
    </nav>
  )
}

/** A problem with no form to show it, such as a sign-out the server did not get; sits under the header. */
export function AuthNotice() {
  const notice = useAuth((state) => state.notice)
  const store = useAuthStore()
  if (notice === null) return null
  return (
    <div className="mx-auto w-full max-w-2xl px-4 pt-4 sm:px-6">
      <Alert variant="destructive">
        <AlertDescription className="text-destructive">{notice}</AlertDescription>
        <AlertAction>
          <Button type="button" variant="ghost" size="xs" onClick={() => store.getState().dismissNotice()}>
            Dismiss
          </Button>
        </AlertAction>
      </Alert>
    </div>
  )
}
