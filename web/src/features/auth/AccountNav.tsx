import { Link, useLocation, useNavigate } from '@tanstack/react-router'
import { ChevronDownIcon, CircleAlertIcon, LogOutIcon } from 'lucide-react'
import { useState } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { PageShell } from '@/components/PageShell'
import { Alert, AlertAction, AlertDescription } from '@/components/ui/alert'
import { Button, buttonVariants } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { cn } from '@/lib/utils'
import { useAuth, useAuthStore } from './context'

// Pages that only mean something while signed in: signing out there goes home instead of to the sign-in form.
const SIGNED_IN_ONLY: ReadonlySet<string> = new Set(['/games', '/decks'])

/**
 * The account part of the header. Nothing while the server is still being asked or when it has no accounts; "Sign
 * in" and "Register" for a guest; for an account a menu named after the username, with "Sign out".
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
        {/* The sign-in page links to registering, which is all a phone-wide header has room for. */}
        <Link
          to="/register"
          className={cn(buttonVariants({ variant: 'outline', size: 'sm' }), 'max-sm:hidden')}
        >
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
    <nav aria-label="Account" className="flex min-w-0 items-center">
      <DropdownMenu>
        <DropdownMenuTrigger render={<Button variant="ghost" size="sm" className="min-w-0 gap-2 px-2" />}>
          <span
            aria-hidden="true"
            className="flex size-6 shrink-0 items-center justify-center rounded-full bg-primary text-xs font-semibold text-on-primary uppercase"
          >
            {user.username.slice(0, 1)}
          </span>
          <span className="min-w-0 max-w-24 truncate max-sm:sr-only lg:max-w-48" title={user.username}>
            <span className="sr-only">Signed in as </span>
            {user.username}
          </span>
          <ChevronDownIcon aria-hidden="true" className="text-muted-foreground max-sm:hidden" />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="min-w-56">
          <DropdownMenuGroup>
            <DropdownMenuLabel>Signed in as</DropdownMenuLabel>
            <p className="px-3 pb-2 text-sm font-medium break-all">{user.username}</p>
          </DropdownMenuGroup>
          <DropdownMenuSeparator />
          <DropdownMenuItem disabled={signingOut} onClick={() => void signOut()}>
            <LogOutIcon aria-hidden="true" />
            Sign out
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </nav>
  )
}

/** A problem with no form to show it, such as a sign-out the server did not get; sits under the header. */
export function AuthNotice() {
  const notice = useAuth((state) => state.notice)
  const store = useAuthStore()
  if (notice === null) return null
  return (
    <PageShell className="pb-0 sm:pb-0">
      <Alert variant="destructive">
        <CircleAlertIcon aria-hidden="true" />
        <AlertDescription className="text-foreground">{notice}</AlertDescription>
        <AlertAction>
          <Button type="button" variant="ghost" size="xs" onClick={() => store.getState().dismissNotice()}>
            Dismiss
          </Button>
        </AlertAction>
      </Alert>
    </PageShell>
  )
}
